//! The derivation of CR 613 as one pass: what the effects of resolved spells and abilities and the static abilities of
//! the permanents make of every permanent of a state.
//!
//! The pass walks the layers in order and applies, one at a time, the effects that belong to a layer to every
//! permanent at once, so an effect reads the other objects as the earlier layers and the earlier effects of its own
//! layer left them. An effect is a continuous effect a spell or ability made or one modification of a static ability
//! a permanent has at that point of the pass (an ability a layer 6 effect took away is not run, one it gave is; CR
//! 613.6). Within a layer the characteristic-defining abilities apply first (CR 613.3); then the effects follow in
//! timestamp order except that an effect that depends on others waits for them (CR 613.8): effect A depends on B when
//! applying B first changes whether A exists, which objects it applies to or what it does to them, and effects that
//! depend on each other in a loop go by timestamp. The pass finds that out by trying B on a copy of the objects it
//! applies to and asking A again, so no card is named; `reads` only rules out the pairs that cannot meet.
//!
//! The result is kept with the state until the next event changes it (`GameState::layers`); `characteristics`,
//! `controller_of`, `keywords_of` and `ability_reader` all read it, so nothing reads an effect another way
//! (`docs/engine.md`, Continuous effects).

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, OnceLock};

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, Modification as Wanted};
use mtg_cards::refs::{Pred, Ref, Selector as Named, ZoneKind};
use mtg_cards::value::CounterKind;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use crate::abilities::{AbilityRef, AbilitySource, Grant};
use crate::ability_reader::{
    definition_has_static, printed, printed_and_provided, provided, working_here,
};
use crate::characteristics::{Characteristics, base};
use crate::effects::{ContinuousEffect, Layer, Modification, Selector};
use crate::events::Event;
use crate::object::{GameObject, ObjectKind, ObjectRef, PermanentState};
use crate::resolve::{Scope, lower, zone_kind};
use crate::state::GameState;
use crate::zones::Zone;

mod reads;

use reads::{
    Traits, reads_of_condition, reads_of_modification, reads_of_ref, writes, writes_of_wanted,
};

/// The layers the pass walks, in the order of CR 613.1 (copy and text-changing effects are not in the engine).
const LAYERS: [Layer; 8] = [
    Layer::Control,
    Layer::Type,
    Layer::Color,
    Layer::Ability,
    Layer::PtCharacteristicDefining,
    Layer::PtSet,
    Layer::PtModify,
    Layer::PtSwitch,
];

thread_local! {
    /// Whether a pass is under way on this thread.
    static COMPUTING: Cell<bool> = const { Cell::new(false) };
    /// The objects outside the battlefield whose characteristic-defining abilities are being worked out.
    static DEFINING: RefCell<Vec<ObjectId>> = const { RefCell::new(Vec::new()) };
    /// What the count under way reads of the objects it looks at, innermost last: an object seen from inside it is
    /// worked out only as far as that.
    static NEEDED: RefCell<Vec<Traits>> = const { RefCell::new(Vec::new()) };
}

#[cfg(test)]
thread_local! {
    /// How many times the pass has been worked out on this thread, for the tests that guard the cost of a crowded table.
    static COMPUTED: Cell<u64> = const { Cell::new(0) };
}

/// Ends the pass of this thread, whether it returned or unwound.
struct Computing;

impl Computing {
    fn enter() -> Computing {
        COMPUTING.with(|computing| computing.set(true));
        Computing
    }
}

impl Drop for Computing {
    fn drop(&mut self) {
        COMPUTING.with(|computing| computing.set(false));
    }
}

/// Takes an object off the list of those being defined, whether the work returned or unwound.
struct Defined;

impl Drop for Defined {
    fn drop(&mut self) {
        DEFINING.with(|open| {
            open.borrow_mut().pop();
        });
    }
}

/// Takes what a count reads off the list, whether the work returned or unwound.
struct Needing;

impl Needing {
    fn enter(reads: Traits) -> Needing {
        NEEDED.with(|open| open.borrow_mut().push(reads));
        Needing
    }
}

impl Drop for Needing {
    fn drop(&mut self) {
        NEEDED.with(|open| {
            open.borrow_mut().pop();
        });
    }
}

/// An ability a permanent has: where it is named and what it says.
#[derive(Debug, Clone)]
pub(crate) struct HeldAbility {
    pub(crate) address: AbilityRef,
    pub(crate) body: Arc<Ability>,
    /// The timestamp it was given at: 0 for a printed ability, the grant's for one that was given (CR 613.7a).
    since: u64,
    /// The keyword that adds it, for an ability a keyword adds: it goes when the keyword does.
    keyword: Option<Keyword>,
}

/// One effect that changed a permanent, in the order the layers applied them.
#[derive(Debug, Clone)]
// `layer` and `timestamp` are read by the tests of the order the effects applied in.
#[allow(dead_code)]
pub(crate) struct Applied {
    /// What the effect is: a resolved spell or ability's, or one modification of a static ability.
    pub(crate) origin: Grant,
    /// The layer it applied in, or none for a restriction.
    pub(crate) layer: Option<Layer>,
    /// Its place in the order of the layer.
    pub(crate) timestamp: u64,
    /// The change.
    pub(crate) modification: Modification,
}

/// What the layers made of a permanent, or what they have made of it so far while the pass is under way.
#[derive(Debug, Clone)]
pub(crate) struct Held {
    id: ObjectId,
    pub(crate) chars: Characteristics,
    pub(crate) controller: PlayerId,
    /// The abilities it has: the printed ones less those lost, then those given, in the order they were given.
    pub(crate) abilities: Vec<HeldAbility>,
    /// The keywords it has, in the order they were gained; one it already has is not listed twice.
    pub(crate) keywords: Vec<Keyword>,
    /// The keywords its keyword counters give with the timestamp each held since: an effect older than the counter does not
    /// take it away (CR 613.7).
    counter_keywords: Vec<(Keyword, u64)>,
    /// The effects that changed it.
    pub(crate) applied: Vec<Applied>,
}

impl Held {
    /// A permanent as its card prints it.
    fn new(object: &GameObject) -> Held {
        let id = object.id();
        let mut abilities = Vec::new();
        let mut keywords = Vec::new();
        for (address, rules) in printed(object) {
            abilities.push(HeldAbility {
                address,
                body: Arc::clone(rules),
                since: 0,
                keyword: None,
            });
            if let Ability::Keyword(keyword) = &**rules {
                keywords.push(keyword.clone());
                let source = AbilitySource::PrintedKeyword(address.ability);
                abilities.extend(provided(id, keyword, source).into_iter().map(
                    |(address, body)| HeldAbility {
                        address,
                        body,
                        since: 0,
                        keyword: Some(keyword.clone()),
                    },
                ));
            }
        }
        Held {
            id,
            chars: base(object),
            controller: object.controller(),
            abilities,
            keywords,
            counter_keywords: Vec::new(),
            applied: Vec::new(),
        }
    }

    /// A card that is not on the battlefield as its card prints it, with the characteristic-defining abilities it has
    /// there: the layers change it like a permanent when a static ability names the zone it is in.
    fn outside(state: &GameState, object: &GameObject) -> Held {
        let mut held = Held::new(object);
        held.chars = outside(state, object, None);
        held
    }

    /// The changes the layers made, applied to `chars`: the characteristics of a card outside the battlefield with its
    /// own defining abilities, then what the effects and static abilities of the game gave it.
    pub(crate) fn replay(&self, chars: &mut Characteristics) {
        for change in &self.applied {
            change.modification.apply(chars);
        }
    }

    /// What is held now, without the record of what changed it but the goad effects, which a selector reads: the copy a
    /// trial changes.
    fn without_history(&self) -> Held {
        Held {
            applied: self
                .applied
                .iter()
                .filter(|applied| matches!(applied.modification, Modification::Goaded { .. }))
                .cloned()
                .collect(),
            ..self.clone()
        }
    }

    /// Gives back the keywords of the keyword counters put on after the effect with the timestamp `stamp` that `named` selects.
    fn keep_newer_counter_keywords(&mut self, stamp: u64, named: impl Fn(&Keyword) -> bool) {
        for (keyword, since) in &self.counter_keywords {
            if *since > stamp && named(keyword) && !self.keywords.contains(keyword) {
                self.keywords.push(keyword.clone());
            }
        }
    }

    /// Makes the change `modification` says, as the effect `origin` with the timestamp `stamp` gives it.
    fn change(&mut self, modification: &Modification, origin: Grant, stamp: u64) {
        modification.apply(&mut self.chars);
        match modification {
            Modification::SetController(player) => self.controller = *player,
            Modification::AddKeyword(keyword) => {
                if !self.keywords.contains(keyword) {
                    self.keywords.push(keyword.clone());
                }
                // Each instance of a keyword adds its abilities, which trigger separately (CR 702.21c, 702.91b,
                // 702.108b), so a keyword the object already has adds them again.
                let source = AbilitySource::GrantedKeyword(origin);
                self.abilities
                    .extend(provided(self.id, keyword, source).into_iter().map(
                        |(address, body)| HeldAbility {
                            address,
                            body,
                            since: stamp,
                            keyword: Some(keyword.clone()),
                        },
                    ));
            }
            Modification::RemoveKeyword(keyword) => {
                self.keywords.retain(|held| held != keyword);
                self.keep_newer_counter_keywords(stamp, |held| held == keyword);
                self.abilities.retain(|held| {
                    held.keyword.as_ref() != Some(keyword)
                        && !matches!(&*held.body, Ability::Keyword(named) if named == keyword)
                });
            }
            Modification::LoseAbilities => {
                self.keywords.clear();
                self.abilities.clear();
                self.keep_newer_counter_keywords(stamp, |_| true);
            }
            Modification::GrantAbility(granted) => self.abilities.push(HeldAbility {
                address: AbilityRef::granted(self.id, origin),
                body: Arc::clone(granted),
                since: stamp,
                keyword: None,
            }),
            Modification::AddType(_)
            | Modification::RemoveType(_)
            | Modification::SetTypes(_)
            | Modification::AddSubtype(_)
            | Modification::SetCreatureTypes(_)
            | Modification::AddAllCreatureTypes
            | Modification::SetColors(_)
            | Modification::SetPowerToughness { .. }
            | Modification::ModifyPowerToughness { .. }
            | Modification::SwitchPowerToughness
            | Modification::CantAttack
            | Modification::CantAttackYou { .. }
            | Modification::CantBlock
            | Modification::DoesntUntap
            | Modification::CantBeTurnedFaceUp
            | Modification::CantBeBlocked
            | Modification::CantBeBlockedBy { .. }
            | Modification::CanBlockOnly { .. }
            | Modification::MinBlockers(_)
            | Modification::MaxBlockers(_)
            | Modification::BlocksAdditional(_)
            | Modification::Goaded { .. } => {}
        }
    }
}

/// What the layers make of every permanent of a state, and the keywords they give to players.
#[derive(Debug)]
pub(crate) struct Layers {
    held: BTreeMap<ObjectId, Held>,
    /// What static abilities give to players ("You have hexproof.").
    players: BTreeMap<PlayerId, PlayerHeld>,
}

/// What the layers gave a player: their keywords, and the record of the effects that changed them.
#[derive(Debug, Clone, Default)]
pub(crate) struct PlayerHeld {
    /// The keywords static abilities give them, in the order they were gained.
    pub(crate) keywords: Vec<Keyword>,
    /// The effects that gave them a keyword or took one, in the order applied.
    pub(crate) applied: Vec<Applied>,
}

impl Layers {
    /// What the layers made of the permanent `id`, or none for an object that is not on the battlefield.
    pub(crate) fn get(&self, id: ObjectId) -> Option<&Held> {
        self.held.get(&id)
    }

    /// The keywords the layers gave to the player `id`.
    pub(crate) fn player_keywords(&self, id: PlayerId) -> &[Keyword] {
        self.players
            .get(&id)
            .map_or(&[], |held| held.keywords.as_slice())
    }

    /// What the layers gave the player `id`, or none while they gave nothing.
    pub(crate) fn player(&self, id: PlayerId) -> Option<&PlayerHeld> {
        self.players.get(&id)
    }
}

/// The reading of a game that has no effect and no static ability, and of one whose pass is under way.
static EMPTY: Layers = Layers {
    held: BTreeMap::new(),
    players: BTreeMap::new(),
};

/// The layers of a state, built when first asked for and dropped when the state changes.
#[derive(Default)]
pub(crate) struct LayersCache(OnceLock<Layers>);

impl LayersCache {
    /// Forgets the result: the state it was built for has changed.
    pub(crate) fn reset(&mut self) {
        self.0 = OnceLock::new();
    }

    /// Forgets the result if there is one.
    pub(crate) fn reset_if_built(&mut self) {
        if self.0.get().is_some() {
            self.reset();
        }
    }
}

// A clone is a state of its own that works out its own layers when it first needs them.
impl Clone for LayersCache {
    fn clone(&self) -> Self {
        LayersCache::default()
    }
}

impl fmt::Debug for LayersCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LayersCache")
    }
}

impl GameState {
    /// What the layers make of every permanent now: nothing while no effect, no static ability and no +1/+1 or -1/-1 counter
    /// can be in play (a game of cards with none costs nothing) and while the pass itself is under way, which reads its own
    /// objects.
    pub(crate) fn layers(&self) -> &Layers {
        let nothing_to_derive =
            self.effects.is_empty() && !self.statics_possible && !self.possible.counters;
        if nothing_to_derive || COMPUTING.with(Cell::get) {
            return &EMPTY;
        }
        self.layers.0.get_or_init(|| {
            #[cfg(test)]
            COMPUTED.with(|count| count.set(count.get() + 1));
            let _computing = Computing::enter();
            compute(self)
        })
    }
}

/// Whether an event leaves what the layers make of the permanents as it was: who has priority, the mana pool, the order
/// and removal of triggers and the journal of a resolution are not among the things the layers read.
pub(crate) fn leaves_alone(event: &Event) -> bool {
    matches!(
        event,
        Event::PriorityGranted { .. }
            | Event::PriorityPassed { .. }
            | Event::ManaAdded { .. }
            | Event::ManaSpent { .. }
            | Event::ManaPoolEmptied { .. }
            | Event::TriggersOrdered { .. }
            | Event::TriggerRemoved { .. }
            | Event::AbilityTriggered { .. }
            | Event::StepDone { .. }
            | Event::ChoiceMade { .. }
            | Event::ResolutionBegan { .. }
    )
}

/// How the objects of the game are read: the pass's objects as they are, and perhaps some of them as a trial changed them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct View<'a> {
    base: &'a BTreeMap<ObjectId, Held>,
    over: Option<&'a BTreeMap<ObjectId, Held>>,
}

impl<'a> View<'a> {
    /// The finished layers of a state.
    pub(crate) fn of(layers: &'a Layers) -> View<'a> {
        View {
            base: &layers.held,
            over: None,
        }
    }

    /// What is held of the permanent `id`.
    pub(crate) fn get(&self, id: ObjectId) -> Option<&'a Held> {
        self.over
            .and_then(|over| over.get(&id))
            .or_else(|| self.base.get(&id))
    }

    /// Every permanent with what is held of it, in id order.
    fn iter(self) -> impl Iterator<Item = (ObjectId, &'a Held)> {
        self.base
            .keys()
            .filter_map(move |id| self.get(*id).map(|held| (*id, held)))
    }
}

/// How a layer finds a candidate again once it has been applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    /// A continuous effect of a resolved spell or ability.
    Effect(EffectId),
    /// The modification `usize` of the static ability at this address.
    Static(AbilityRef, usize),
}

/// A static ability's modification, with what it needs to be worked out again.
#[derive(Debug, Clone)]
struct StaticCand {
    /// The permanent with the ability.
    holder: ObjectRef,
    address: AbilityRef,
    body: Arc<Ability>,
    /// The modification's index among the ability's.
    index: usize,
}

/// Where a candidate comes from.
enum Source<'a> {
    Registered(&'a ContinuousEffect),
    Static(StaticCand),
}

/// An effect waiting to apply in the layer being worked through.
struct Cand<'a> {
    key: Key,
    layer: Option<Layer>,
    /// Whether it comes from a characteristic-defining ability, which applies first and depends on none but its kind.
    defining: bool,
    timestamp: u64,
    origin: Grant,
    what: Source<'a>,
    /// The static ability it is a modification of: its modifications apply to the same permanents (CR 613.6), so none
    /// depends on another.
    ability: Option<AbilityRef>,
    /// What it reads of the game and what it writes, which only a layer with several effects needs to know: it is a pair of
    /// them that can depend on each other.
    reads: Traits,
    writes: Traits,
    known: bool,
}

impl Cand<'_> {
    /// The order effects apply in when nothing depends on anything: the characteristic-defining abilities first (CR
    /// 613.3), then by timestamp, registered effects by id before static ones by source, ability and modification.
    fn order(&self) -> (bool, u64, Key) {
        (!self.defining, self.timestamp, self.key)
    }
}

/// What an effect does now: the change, and the permanents it makes it to.
#[derive(Debug, Clone, PartialEq)]
struct Foot {
    modification: Modification,
    set: BTreeSet<ObjectId>,
    /// The players it gives a keyword to or takes one from.
    players: BTreeSet<PlayerId>,
}

/// The effects of a static ability that began to apply in an earlier layer: they go on applying to the permanents they
/// began with, whatever has happened to the ability since (CR 613.6).
struct Started {
    holder: ObjectRef,
    body: Arc<Ability>,
    /// The timestamp its effects had (CR 613.7a).
    stamp: u64,
    set: BTreeSet<ObjectId>,
    players: BTreeSet<PlayerId>,
}

/// Whether a static ability's `affected` names players: the keywords it changes are the players'.
fn is_player_ref(affected: &Ref) -> bool {
    matches!(
        affected,
        Ref::You | Ref::Opponents | Ref::EachPlayer | Ref::AttachedPlayer
    )
}

/// The layer a modification of the data belongs to, or none for a restriction.
fn layer_of(wanted: &Wanted, defining: bool) -> Option<Layer> {
    Some(match wanted {
        Wanted::SetController(_) => Layer::Control,
        Wanted::AddType(_)
        | Wanted::RemoveType(_)
        | Wanted::SetTypes(_)
        | Wanted::RemoveSupertype(_)
        | Wanted::AddSubtype(_)
        | Wanted::SetCreatureTypes(_)
        | Wanted::AddAllCreatureTypes => Layer::Type,
        Wanted::SetColors(_) => Layer::Color,
        Wanted::AddKeyword(_)
        | Wanted::AddProtectionFromChosenColor
        | Wanted::RemoveKeyword(_)
        | Wanted::GrantAbility(_)
        | Wanted::LoseAbilities => Layer::Ability,
        Wanted::SetPT { .. } if defining => Layer::PtCharacteristicDefining,
        Wanted::SetPT { .. } => Layer::PtSet,
        Wanted::ModifyPT { .. } => Layer::PtModify,
        Wanted::SwitchPT => Layer::PtSwitch,
        Wanted::CantAttack
        | Wanted::CantAttackYou(_)
        | Wanted::CantBlock
        | Wanted::DoesntUntap
        | Wanted::CantBeTurnedFaceUp
        | Wanted::CantBeBlocked
        | Wanted::CantBeBlockedBy(_)
        | Wanted::CanBlockOnly(_)
        | Wanted::MinBlockers(_)
        | Wanted::MaxBlockers(_)
        | Wanted::BlocksAdditional(_)
        | Wanted::Goaded => return None,
    })
}

/// The zone a selector looks in: the one it names, the stack for spells, else the battlefield.
pub(crate) fn zone_named(selector: &Named) -> ZoneKind {
    selector
        .0
        .iter()
        .find_map(|pred| match pred {
            Pred::In(kind) => Some(*kind),
            _ => None,
        })
        .unwrap_or_else(|| {
            if selector.0.contains(&Pred::Spell) {
                ZoneKind::Stack
            } else {
                ZoneKind::Battlefield
            }
        })
}

/// The zones other than the battlefield that a static ability of the game names in the selector of what it affects:
/// the cards there are held by the pass like permanents ("Each creature card in your hand has cycling {1}{U}").
fn outside_zones(state: &GameState) -> Vec<ZoneKind> {
    let mut zones = Vec::new();
    for object in state.objects() {
        let here = object.zone() == Zone::Battlefield;
        if object.kind() == ObjectKind::Ability
            || !definition_has_static(object.card())
            || (!here && !state.possible.elsewhere)
        {
            continue;
        }
        let abilities: Vec<&Arc<Ability>> = if here {
            printed(object).map(|(_, rules)| rules).collect()
        } else {
            working_here(object).map(|(_, rules)| rules).collect()
        };
        for rules in abilities {
            if let Ability::Static {
                affected: Ref::Each(selector),
                ..
            } = &**rules
            {
                let zone = zone_named(selector);
                if zone != ZoneKind::Battlefield && !zones.contains(&zone) {
                    zones.push(zone);
                }
            }
        }
    }
    zones
}

/// The pass: the permanents as the layers have left them so far, and what has been applied.
struct Pass<'a> {
    state: &'a GameState,
    held: BTreeMap<ObjectId, Held>,
    players: BTreeMap<PlayerId, PlayerHeld>,
    /// The continuous effects of resolved spells and abilities that still apply.
    registered: Vec<&'a ContinuousEffect>,
    started: BTreeMap<AbilityRef, Started>,
    done: BTreeSet<Key>,
}

/// Works out what the layers make of every permanent of `state`.
///
/// An effect that lasts "for as long as you control ~" ends when its source is no longer controlled by the player of the
/// effect, and who controls the source is itself a result of this pass. The pass therefore applies all such effects
/// first, then drops the earliest one whose source ended up under another player's control and works the pass again
/// without it (a dropped effect never comes back, so this ends), so that the check reads the control as the layers left
/// it and never the control as a stale state records it (CR 611.2b, 613.7).
fn compute(state: &GameState) -> Layers {
    let mut registered: Vec<&ContinuousEffect> = state
        .effects()
        .iter()
        .filter(|effect| effect.is_active_before_control(state))
        .collect();
    loop {
        let layers = compute_with(state, &registered);
        let lost = registered.iter().position(|effect| {
            effect.duration == Duration::WhileYouControlSource
                && !effect.source.is_some_and(|source| {
                    layers
                        .get(source.id)
                        .map(|held| held.controller)
                        .or_else(|| state.object(source.id).map(GameObject::controller))
                        == Some(effect.controller)
                })
        });
        match lost {
            Some(at) => {
                registered.remove(at);
            }
            None => return layers,
        }
    }
}

/// The pass over these effects (and the static abilities of the permanents).
fn compute_with(state: &GameState, registered: &[&ContinuousEffect]) -> Layers {
    let registered: Vec<&ContinuousEffect> = registered.to_vec();
    // With no effect and no static ability that works where its card is, every permanent is as its card prints it.
    let statics_in_play = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .any(|object| definition_has_static(object.card()))
        || (state.possible.elsewhere
            && state.objects().any(|object| {
                object.zone() != Zone::Battlefield
                    && object.kind() != ObjectKind::Ability
                    && working_here(object)
                        .any(|(_, rules)| matches!(**rules, Ability::Static { .. }))
            }));
    let counters_in_play = state.possible.counters
        && state
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| state.object(*id)?.permanent())
            .any(|permanent| {
                power_toughness_from_counters(permanent) != 0 || has_keyword_counter(permanent)
            });
    if registered.is_empty() && !statics_in_play && !counters_in_play {
        return Layers {
            held: BTreeMap::new(),
            players: BTreeMap::new(),
        };
    }
    let mut held = BTreeMap::new();
    for &id in state.zone(Zone::Battlefield) {
        if let Some(object) = state.object(id)
            && object.kind() != ObjectKind::Ability
        {
            held.insert(id, Held::new(object));
        }
    }
    let zones = outside_zones(state);
    if !zones.is_empty() {
        for object in state.objects() {
            if object.kind() != ObjectKind::Ability
                && object.zone() != Zone::Battlefield
                && object.zone() != Zone::PhasedOut
                && zones.contains(&zone_kind(object.zone()))
            {
                held.insert(object.id(), Held::outside(state, object));
            }
        }
    }
    let mut pass = Pass {
        state,
        held,
        players: BTreeMap::new(),
        registered,
        started: BTreeMap::new(),
        done: BTreeSet::new(),
    };
    for layer in LAYERS {
        pass.run(Some(layer));
    }
    pass.run(None);
    Layers {
        held: pass.held,
        players: pass.players,
    }
}

impl<'a> Pass<'a> {
    /// Applies the effects of one layer (none: the restrictions, which come last), one at a time, each chosen again
    /// from what is left once the one before has applied (CR 613.8c).
    fn run(&mut self, layer: Option<Layer>) {
        if layer == Some(Layer::Ability) {
            self.apply_keyword_counters();
        }
        if layer == Some(Layer::PtModify) {
            self.apply_counters();
        }
        let mut waiting = self.candidates(layer);
        Self::order(&mut waiting);
        while !waiting.is_empty() {
            let at = self.next(&waiting);
            let chosen = waiting.remove(at);
            if self.apply(&chosen) {
                // It gave a static ability, whose modifications of this layer join the others.
                for found in self.candidates(layer) {
                    if !waiting.iter().any(|other| other.key == found.key) {
                        waiting.push(found);
                    }
                }
                Self::order(&mut waiting);
            }
        }
    }

    /// The keywords the keyword counters give the permanents (CR 122.1b, 613.1f): they apply first in layer 6, so an
    /// effect that takes the keyword or the abilities away applies after them, and a keyword from two counters or from
    /// a counter and the card is held once.
    fn apply_keyword_counters(&mut self) {
        let state = self.state;
        for (id, held) in &mut self.held {
            let Some(permanent) = state.object(*id).and_then(GameObject::permanent) else {
                continue;
            };
            for (kind, _) in permanent.counters() {
                let Some(keyword) = kind.keyword() else {
                    continue;
                };
                let since = permanent.keyword_stamps.get(kind).copied().unwrap_or(0);
                if !held.keywords.contains(&keyword) {
                    held.keywords.push(keyword.clone());
                }
                held.counter_keywords.push((keyword, since));
            }
        }
    }

    /// The power and toughness the counters give the permanents (CR 613.4c): they apply before the other effects of layer
    /// 7c, which then read the result, and after everything that sets power and toughness in layers 7a and 7b.
    fn apply_counters(&mut self) {
        let state = self.state;
        for (id, held) in &mut self.held {
            let Some(permanent) = state.object(*id).and_then(GameObject::permanent) else {
                continue;
            };
            let net = power_toughness_from_counters(permanent);
            if net != 0 {
                held.chars.power = held.chars.power.map(|power| power.saturating_add(net));
                held.chars.toughness = held
                    .chars
                    .toughness
                    .map(|toughness| toughness.saturating_add(net));
            }
        }
    }

    /// Puts the waiting effects in the order they apply in when nothing depends on anything, and finds out what each
    /// reads and writes when there are several.
    fn order(waiting: &mut [Cand<'a>]) {
        waiting.sort_by_key(Cand::order);
        if waiting.len() > 1 {
            for cand in waiting.iter_mut().filter(|cand| !cand.known) {
                cand.reads = Self::reads(cand);
                cand.writes = writes_of(cand);
                cand.known = true;
            }
        }
    }

    /// The effects of `layer` that have not applied: those of resolved spells and abilities, and the modifications of
    /// the static abilities the permanents have now (the ones that began to apply earlier included).
    fn candidates(&self, layer: Option<Layer>) -> Vec<Cand<'a>> {
        let mut found = Vec::new();
        for &effect in &self.registered {
            let key = Key::Effect(effect.id);
            if effect.layer == layer && !self.done.contains(&key) {
                found.push(Cand {
                    key,
                    layer,
                    defining: false,
                    timestamp: effect.timestamp,
                    origin: Grant::Effect(effect.id),
                    what: Source::Registered(effect),
                    ability: None,
                    reads: Traits::NONE,
                    writes: Traits::NONE,
                    known: false,
                });
            }
        }
        for (id, held) in &self.held {
            let Some(object) = self.state.object(*id) else {
                continue;
            };
            let Some(permanent) = object.permanent() else {
                continue;
            };
            for ability in &held.abilities {
                self.static_candidates(
                    &mut found,
                    layer,
                    (object.reference(), ZoneKind::Battlefield),
                    permanent.timestamp(),
                    ability,
                );
            }
        }
        // A static ability of a card that is not on the battlefield applies while the card is in the zone it names; its
        // timestamp is 0, since only permanents are stamped.
        if self.state.possible.elsewhere {
            for object in self.state.objects() {
                if object.zone() == Zone::Battlefield || object.kind() == ObjectKind::Ability {
                    continue;
                }
                for (address, rules) in working_here(object) {
                    let ability = HeldAbility {
                        address,
                        body: Arc::clone(rules),
                        since: 0,
                        keyword: None,
                    };
                    self.static_candidates(
                        &mut found,
                        layer,
                        (object.reference(), zone_kind(object.zone())),
                        0,
                        &ability,
                    );
                }
            }
        }
        for (address, started) in &self.started {
            let held = self.held.get(&address.object).is_some_and(|held| {
                held.abilities
                    .iter()
                    .any(|ability| ability.address == *address)
            });
            if !held {
                let kept = HeldAbility {
                    address: *address,
                    body: Arc::clone(&started.body),
                    since: 0,
                    keyword: None,
                };
                self.static_candidates(
                    &mut found,
                    layer,
                    (started.holder, ZoneKind::Battlefield),
                    started.stamp,
                    &kept,
                );
            }
        }
        found
    }

    /// The modifications of the static ability `ability` of `holder`, which is in the zone `zone`, that belong to `layer`.
    fn static_candidates(
        &self,
        found: &mut Vec<Cand<'a>>,
        layer: Option<Layer>,
        (holder, zone): (ObjectRef, ZoneKind),
        entered: u64,
        ability: &HeldAbility,
    ) {
        let Ability::Static {
            mods,
            defining,
            from,
            ..
        } = &*ability.body
        else {
            return;
        };
        // An ability applies only while its source is where it works from.
        if *from != zone {
            return;
        }
        for (index, wanted) in mods.iter().enumerate() {
            let key = Key::Static(ability.address, index);
            if layer_of(wanted, *defining) != layer || self.done.contains(&key) {
                continue;
            }
            let origin = match ability.address.source {
                AbilitySource::Printed => Grant::Static {
                    source: holder.id,
                    rule: ability.address.ability,
                    index,
                },
                AbilitySource::Granted(grant) | AbilitySource::GrantedKeyword(grant) => grant,
                // A static ability a keyword adds (reconfigure) is named by the rule of the keyword on the card.
                AbilitySource::PrintedKeyword(rule) => Grant::Static {
                    source: holder.id,
                    rule,
                    index,
                },
            };
            found.push(Cand {
                key,
                layer,
                defining: *defining,
                // CR 613.7a: the later of the permanent's own and the time the ability was given.
                timestamp: entered.max(ability.since),
                origin,
                what: Source::Static(StaticCand {
                    holder,
                    address: ability.address,
                    body: Arc::clone(&ability.body),
                    index,
                }),
                ability: Some(ability.address),
                reads: Traits::NONE,
                writes: Traits::NONE,
                known: false,
            });
        }
    }

    /// What the effect does in the world `view`, or none when it does not exist there: a static ability its source no
    /// longer has, or whose condition fails, or that cannot be worked out.
    fn footprint(&self, view: View<'_>, cand: &Cand<'_>) -> Option<Foot> {
        match &cand.what {
            Source::Registered(effect) => {
                let set = match &effect.selector {
                    Selector::Objects(named) => named
                        .iter()
                        .filter(|reference| {
                            view.get(reference.id).is_some()
                                && self
                                    .state
                                    .object(reference.id)
                                    .is_some_and(|object| object.reference() == **reference)
                        })
                        .map(|reference| reference.id)
                        .collect(),
                    selector => view
                        .iter()
                        .filter(|(id, held)| {
                            self.state.object(*id).is_some_and(|object| {
                                object.zone() == Zone::Battlefield
                                    && selector.matches(object, &held.chars, held.controller)
                            })
                        })
                        .map(|(id, _)| id)
                        .collect(),
                };
                Some(Foot {
                    modification: effect.modification.clone(),
                    set,
                    players: BTreeSet::new(),
                })
            }
            Source::Static(cand) => self.static_footprint(view, cand),
        }
    }

    fn static_footprint(&self, view: View<'_>, cand: &StaticCand) -> Option<Foot> {
        let Ability::Static {
            affected,
            mods,
            condition,
            ..
        } = &*cand.body
        else {
            return None;
        };
        let started = self.started.get(&cand.address);
        let controller = if let Some(source) = view.get(cand.holder.id) {
            if started.is_none()
                && !source
                    .abilities
                    .iter()
                    .any(|ability| ability.address == cand.address)
            {
                return None;
            }
            source.controller
        } else {
            // A card elsewhere has the abilities it prints, and its owner is the player "you" names.
            self.state
                .object(cand.holder.id)
                .filter(|object| {
                    object.reference() == cand.holder && object.zone() != Zone::Battlefield
                })?
                .controller()
        };
        let scope = Scope::announcing(self.state, controller, cand.holder).with_layered(view);
        let players = if let Some(started) = started {
            started.players.clone()
        } else if is_player_ref(affected) {
            scope
                .players(affected)
                .map_or_else(|_| BTreeSet::new(), |named| named.into_iter().collect())
        } else {
            BTreeSet::new()
        };
        let set = if let Some(started) = started {
            started.set.clone()
        } else {
            if let Some(condition) = condition
                && !scope.holds(condition).unwrap_or(false)
            {
                return None;
            }
            match affected {
                Ref::This => BTreeSet::from([cand.holder.id]),
                Ref::Attached => {
                    let host = self.state.host_of(cand.holder.id)?;
                    view.get(host)?;
                    BTreeSet::from([host])
                }
                Ref::Each(selector) => {
                    let zone = zone_named(selector);
                    view.iter()
                        .filter(|(id, held)| {
                            self.state.object(*id).is_some_and(|object| {
                                zone_kind(object.zone()) == zone
                                    && scope.selects_with(selector, object, held).unwrap_or(false)
                            })
                        })
                        .map(|(id, _)| id)
                        .collect()
                }
                // The data check keeps every other reference out of a static ability.
                _ => BTreeSet::new(),
            }
        };
        let wanted = mods.get(cand.index)?;
        let modification = {
            let _reading = Needing::enter(reads_of_modification(wanted));
            lower(&scope, wanted).ok()?
        };
        Some(Foot {
            modification,
            set,
            players,
        })
    }

    /// Applies the effect to every permanent it applies to, and notes that a static ability has begun to apply. Whether
    /// what it did was to give a static ability.
    fn apply(&mut self, cand: &Cand<'_>) -> bool {
        self.done.insert(cand.key);
        let view = View {
            base: &self.held,
            over: None,
        };
        let Some(foot) = self.footprint(view, cand) else {
            return false;
        };
        if let Source::Static(this) = &cand.what {
            self.started.entry(this.address).or_insert_with(|| Started {
                holder: this.holder,
                body: Arc::clone(&this.body),
                stamp: cand.timestamp,
                set: foot.set.clone(),
                players: foot.players.clone(),
            });
        }
        for player in &foot.players {
            self.give_player(*player, &foot.modification);
            self.players
                .entry(*player)
                .or_default()
                .applied
                .push(Applied {
                    origin: cand.origin,
                    layer: cand.layer,
                    timestamp: cand.timestamp,
                    modification: foot.modification.clone(),
                });
        }
        for id in &foot.set {
            if let Some(held) = self.held.get_mut(id) {
                held.change(&foot.modification, cand.origin, cand.timestamp);
                held.applied.push(Applied {
                    origin: cand.origin,
                    layer: cand.layer,
                    timestamp: cand.timestamp,
                    modification: foot.modification.clone(),
                });
            }
        }
        matches!(&foot.modification, Modification::GrantAbility(granted)
            if matches!(&**granted, Ability::Static { .. }))
            && !foot.set.is_empty()
    }

    /// A keyword a static ability gives to a player or takes from them (layer 6).
    fn give_player(&mut self, player: PlayerId, modification: &Modification) {
        match modification {
            Modification::AddKeyword(keyword) => {
                let held = &mut self.players.entry(player).or_default().keywords;
                if !held.contains(keyword) {
                    held.push(keyword.clone());
                }
            }
            Modification::RemoveKeyword(keyword) => {
                if let Some(held) = self.players.get_mut(&player) {
                    held.keywords.retain(|kept| kept != keyword);
                }
            }
            _ => {}
        }
    }

    /// Which of the waiting effects applies next: the first, by timestamp, that depends on no other effect that does
    /// not depend on it in turn. The characteristic-defining abilities go before the others, and neither kind
    /// depends on the other (CR 613.3, 613.8a). `waiting` is in the order of `order`.
    fn next(&self, waiting: &[Cand<'_>]) -> usize {
        let n = waiting
            .iter()
            .take_while(|cand| cand.defining == waiting[0].defining)
            .count();
        // Most layers have no two effects where one writes what the other reads, so none can depend on another.
        let may = |a: usize, b: usize| {
            a != b
                && waiting[a].reads.meets(waiting[b].writes)
                && (waiting[a].ability.is_none() || waiting[a].ability != waiting[b].ability)
        };
        if n == 1 || !(0..n).any(|a| (0..n).any(|b| may(a, b))) {
            return 0;
        }
        let mut depends = vec![vec![false; n]; n];
        let mut any = false;
        let now = View {
            base: &self.held,
            over: None,
        };
        // What each effect does now, worked out when something is found that could change it.
        let mut before: Vec<Option<Option<Foot>>> = vec![None; n];
        for b in 0..n {
            let readers: Vec<usize> = (0..n).filter(|&a| may(a, b)).collect();
            if readers.is_empty() {
                continue;
            }
            // `b` applied to what it applies to, on copies of those permanents.
            let Some(by_b) = self
                .footprint(now, &waiting[b])
                .filter(|foot| !foot.set.is_empty())
            else {
                continue;
            };
            let changed: BTreeMap<ObjectId, Held> = by_b
                .set
                .iter()
                .filter_map(|id| {
                    let mut trial = self.held.get(id)?.without_history();
                    trial.change(&by_b.modification, waiting[b].origin, waiting[b].timestamp);
                    trial.applied.push(Applied {
                        origin: waiting[b].origin,
                        layer: waiting[b].layer,
                        timestamp: waiting[b].timestamp,
                        modification: by_b.modification.clone(),
                    });
                    Some((*id, trial))
                })
                .collect();
            let after_b = View {
                base: &self.held,
                over: Some(&changed),
            };
            for a in readers {
                let was = before[a].get_or_insert_with(|| self.footprint(now, &waiting[a]));
                // CR 613.8a: `a` depends on `b` when this changes whether `a` exists, what it applies to or what it does.
                if *was != self.footprint(after_b, &waiting[a]) {
                    depends[a][b] = true;
                    any = true;
                }
            }
        }
        if !any {
            return 0;
        }
        // Who depends on whom, through any number of effects: two that reach each other are in a loop, which goes by
        // timestamp (CR 613.8b).
        let mut reach = depends.clone();
        for k in 0..n {
            let through = reach[k].clone();
            for row in &mut reach {
                if row[k] {
                    for (cell, via) in row.iter_mut().zip(&through) {
                        *cell |= *via;
                    }
                }
            }
        }
        (0..n)
            .find(|&i| (0..n).all(|j| j == i || !depends[i][j] || reach[j][i]))
            .unwrap_or(0)
    }

    /// What the effect reads of the game to work out whether it exists, what it applies to and what it does.
    fn reads(cand: &Cand<'_>) -> Traits {
        match &cand.what {
            Source::Registered(effect) => match &effect.selector {
                Selector::Objects(_) => Traits::NONE,
                Selector::AllCreatures => Traits::TYPES,
                Selector::CreaturesControlledBy(_) => Traits::TYPES.or(Traits::CONTROL),
            },
            Source::Static(cand) => {
                let Ability::Static {
                    affected,
                    mods,
                    condition,
                    ..
                } = &*cand.body
                else {
                    return Traits::NONE;
                };
                // The ability itself, and the controller of its source, whom "you" names.
                let own = Traits::RULES;
                let wanted = mods
                    .get(cand.index)
                    .map_or(Traits::NONE, reads_of_modification);
                let when = condition.as_ref().map_or(Traits::NONE, reads_of_condition);
                own.or(reads_of_ref(affected)).or(when).or(wanted)
            }
        }
    }
}

/// Whether a keyword counter is on the permanent.
fn has_keyword_counter(permanent: &PermanentState) -> bool {
    permanent
        .counters()
        .any(|(kind, _)| kind.keyword().is_some())
}

/// What the +1/+1 and -1/-1 counters on a permanent add to its power and to its toughness: one for each +1/+1 counter and
/// minus one for each -1/-1 counter.
fn power_toughness_from_counters(permanent: &PermanentState) -> i32 {
    let plus = i64::from(permanent.counters_of(&CounterKind::PlusOnePlusOne));
    let minus = i64::from(permanent.counters_of(&CounterKind::MinusOneMinusOne));
    i32::try_from(plus - minus).unwrap_or(if plus > minus { i32::MAX } else { i32::MIN })
}

/// What the effect changes in the objects it applies to.
fn writes_of(cand: &Cand<'_>) -> Traits {
    match &cand.what {
        Source::Registered(effect) => writes(&effect.modification),
        Source::Static(cand) => match &*cand.body {
            Ability::Static { mods, .. } => {
                mods.get(cand.index).map_or(Traits::NONE, writes_of_wanted)
            }
            _ => Traits::NONE,
        },
    }
}

/// The characteristics of an object that is not on the battlefield: the ones its card prints, changed by the
/// characteristic-defining abilities of the card, which work in every zone (CR 604.3).
///
/// `view` says how the permanents they count are read: the pass in progress, or none when the finished layers of the
/// state are asked for. An object whose abilities are under way is read as printed, so abilities that count each
/// other's objects end.
pub(crate) fn outside(
    state: &GameState,
    object: &GameObject,
    view: Option<View<'_>>,
) -> Characteristics {
    let mut chars = base(object);
    let abilities = printed_and_provided(object);
    let mut changes: Vec<(Layer, usize, usize, &Wanted)> = Vec::new();
    // Seen from inside the count of another ability, an object is worked out only as far as that count reads it: the
    // number of creature cards in graveyards reads their types, not the power another such card defines for itself,
    // and working that out for every order the cards can be counted in takes the work beyond reach.
    let needed = NEEDED.with(|open| open.borrow().last().copied());
    for (address, rules) in &abilities {
        let rule = address.ability;
        if let Ability::Static {
            mods,
            defining: true,
            ..
        } = &**rules
        {
            for (index, wanted) in mods.iter().enumerate() {
                if let Some(layer) = layer_of(wanted, true)
                    && needed.is_none_or(|needed| writes_of_wanted(wanted).meets(needed))
                {
                    changes.push((layer, rule, index, wanted));
                }
            }
        }
    }
    if changes.is_empty() || DEFINING.with(|open| open.borrow().contains(&object.id())) {
        return chars;
    }
    DEFINING.with(|open| open.borrow_mut().push(object.id()));
    let _defined = Defined;
    changes.sort_by_key(|(layer, rule, index, _)| (*layer, *rule, *index));
    let view = view.unwrap_or_else(|| View::of(state.layers()));
    let scope =
        Scope::announcing(state, object.controller(), object.reference()).with_layered(view);
    for (_, _, _, wanted) in changes {
        let lowered = {
            let _reading = Needing::enter(reads_of_modification(wanted));
            lower(&scope, wanted)
        };
        if let Ok(modification) = lowered {
            modification.apply(&mut chars);
        }
    }
    chars
}

#[cfg(test)]
mod c4_control;
#[cfg(test)]
mod characteristic_keywords;
#[cfg(test)]
mod chosen;
#[cfg(test)]
mod counters;
#[cfg(test)]
mod crowd;
#[cfg(test)]
mod deck_auras;
#[cfg(test)]
mod defining;
#[cfg(test)]
mod dependencies;
#[cfg(test)]
mod keyword_counters;
#[cfg(test)]
mod outside;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod w8a_goaded;
