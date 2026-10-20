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
use mtg_cards::modify::Modification as Wanted;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use crate::abilities::{AbilityRef, AbilitySource, Grant};
use crate::ability_reader::{printed, provided, working_here};
use crate::characteristics::{Characteristics, base};
use crate::effects::{ContinuousEffect, Layer, Modification, Selector};
use crate::events::Event;
use crate::object::{GameObject, ObjectKind, ObjectRef};
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
            applied: Vec::new(),
        }
    }

    /// What is held now, without the record of what changed it: the copy a trial changes.
    fn without_history(&self) -> Held {
        Held {
            applied: Vec::new(),
            ..self.clone()
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
                self.abilities.retain(|held| {
                    held.keyword.as_ref() != Some(keyword)
                        && !matches!(&*held.body, Ability::Keyword(named) if named == keyword)
                });
            }
            Modification::LoseAbilities => {
                self.keywords.clear();
                self.abilities.clear();
            }
            Modification::GrantAbility(granted) => self.abilities.push(HeldAbility {
                address: AbilityRef::granted(self.id, origin),
                body: Arc::clone(granted),
                since: stamp,
                keyword: None,
            }),
            Modification::AddType(_)
            | Modification::RemoveType(_)
            | Modification::AddSubtype(_)
            | Modification::SetColors(_)
            | Modification::SetPowerToughness { .. }
            | Modification::ModifyPowerToughness { .. }
            | Modification::SwitchPowerToughness
            | Modification::CantAttack
            | Modification::CantBlock => {}
        }
    }
}

/// What the layers make of every permanent of a state.
#[derive(Debug)]
pub(crate) struct Layers {
    held: BTreeMap<ObjectId, Held>,
}

impl Layers {
    /// What the layers made of the permanent `id`, or none for an object that is not on the battlefield.
    pub(crate) fn get(&self, id: ObjectId) -> Option<&Held> {
        self.held.get(&id)
    }
}

/// The reading of a game that has no effect and no static ability, and of one whose pass is under way.
static EMPTY: Layers = Layers {
    held: BTreeMap::new(),
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
    /// What the layers make of every permanent now: nothing while no effect and no static ability can be in play (a game
    /// of cards with none costs nothing) and while the pass itself is under way, which reads its own objects.
    pub(crate) fn layers(&self) -> &Layers {
        if (self.effects.is_empty() && !self.statics_possible) || COMPUTING.with(Cell::get) {
            return &EMPTY;
        }
        self.layers.0.get_or_init(|| {
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
}

/// The effects of a static ability that began to apply in an earlier layer: they go on applying to the permanents they
/// began with, whatever has happened to the ability since (CR 613.6).
struct Started {
    holder: ObjectRef,
    body: Arc<Ability>,
    /// The timestamp its effects had (CR 613.7a).
    stamp: u64,
    set: BTreeSet<ObjectId>,
}

/// The layer a modification of the data belongs to, or none for a restriction.
fn layer_of(wanted: &Wanted, defining: bool) -> Option<Layer> {
    Some(match wanted {
        Wanted::SetController(_) => Layer::Control,
        Wanted::AddType(_) | Wanted::RemoveType(_) | Wanted::AddSubtype(_) => Layer::Type,
        Wanted::SetColors(_) => Layer::Color,
        Wanted::AddKeyword(_)
        | Wanted::RemoveKeyword(_)
        | Wanted::GrantAbility(_)
        | Wanted::LoseAbilities => Layer::Ability,
        Wanted::SetPT { .. } if defining => Layer::PtCharacteristicDefining,
        Wanted::SetPT { .. } => Layer::PtSet,
        Wanted::ModifyPT { .. } => Layer::PtModify,
        Wanted::SwitchPT => Layer::PtSwitch,
        Wanted::CantAttack | Wanted::CantBlock => return None,
    })
}

/// The pass: the permanents as the layers have left them so far, and what has been applied.
struct Pass<'a> {
    state: &'a GameState,
    held: BTreeMap<ObjectId, Held>,
    /// The continuous effects of resolved spells and abilities that still apply.
    registered: Vec<&'a ContinuousEffect>,
    started: BTreeMap<AbilityRef, Started>,
    done: BTreeSet<Key>,
}

/// Works out what the layers make of every permanent of `state`.
fn compute(state: &GameState) -> Layers {
    let registered: Vec<&ContinuousEffect> = state
        .effects()
        .iter()
        .filter(|effect| effect.is_active(state))
        .collect();
    // With no effect and no static ability that works where its card is, every permanent is as its card prints it.
    let statics_in_play = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .any(|object| printed(object).any(|(_, rules)| matches!(**rules, Ability::Static { .. })))
        || (state.zone_abilities_possible
            && state.objects().any(|object| {
                object.zone() != Zone::Battlefield
                    && object.kind() != ObjectKind::Ability
                    && working_here(object).any(|(_, rules)| matches!(**rules, Ability::Static { .. }))
            }));
    if registered.is_empty() && !statics_in_play {
        return Layers {
            held: BTreeMap::new(),
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
    let mut pass = Pass {
        state,
        held,
        registered,
        started: BTreeMap::new(),
        done: BTreeSet::new(),
    };
    for layer in LAYERS {
        pass.run(Some(layer));
    }
    pass.run(None);
    Layers { held: pass.held }
}

impl<'a> Pass<'a> {
    /// Applies the effects of one layer (none: the restrictions, which come last), one at a time, each chosen again
    /// from what is left once the one before has applied (CR 613.8c).
    fn run(&mut self, layer: Option<Layer>) {
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
        if self.state.zone_abilities_possible {
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
                AbilitySource::Granted(grant) => grant,
                // No keyword adds a static ability, so none is applied; one that did would need a grant of its own.
                AbilitySource::PrintedKeyword(_) | AbilitySource::GrantedKeyword(_) => continue,
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
                                selector.matches(object, &held.chars, held.controller)
                            })
                        })
                        .map(|(id, _)| id)
                        .collect(),
                };
                Some(Foot {
                    modification: effect.modification.clone(),
                    set,
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
                Ref::Each(selector) => view
                    .iter()
                    .filter(|(id, held)| {
                        self.state.object(*id).is_some_and(|object| {
                            scope.selects_with(selector, object, held).unwrap_or(false)
                        })
                    })
                    .map(|(id, _)| id)
                    .collect(),
                // The data check keeps every other reference out of a static ability.
                _ => BTreeSet::new(),
            }
        };
        let modification = lower(&scope, mods.get(cand.index)?).ok()?;
        Some(Foot { modification, set })
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
    let mut changes: Vec<(Layer, usize, usize, &Wanted)> = Vec::new();
    for (address, rules) in printed(object) {
        let rule = address.ability;
        if let Ability::Static {
            mods,
            defining: true,
            ..
        } = &**rules
        {
            for (index, wanted) in mods.iter().enumerate() {
                if let Some(layer) = layer_of(wanted, true) {
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
        if let Ok(modification) = lower(&scope, wanted) {
            modification.apply(&mut chars);
        }
    }
    chars
}

#[cfg(test)]
mod defining;
#[cfg(test)]
mod dependencies;
#[cfg(test)]
mod tests;
