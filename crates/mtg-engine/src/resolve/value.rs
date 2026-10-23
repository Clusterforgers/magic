//! The values of the DSL read from the game: references, selectors, amounts and conditions.
//!
//! Nothing here changes state. What the engine cannot evaluate yet (devotion) is a
//! `ResolveError::Unsupported` with the name of what is missing; `Caps` keeps the cards that need
//! them from being offered, so the error is not met in a game.

use std::borrow::Cow;
use std::collections::BTreeMap;

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{AbilityKind, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_cards::value::{Amount, CastWay, Condition, CounterKind, StatKind};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use thiserror::Error;

use super::share::{OUTSIDE_A_DIVISION, Share};
use crate::abilities::Cause;
use crate::announce::payments;
use crate::characteristics::{Characteristics, characteristics, controller_of, seen_as};
use crate::choose::legal_from;
use crate::choose::{Choices, Target};
use crate::combat::AttackTarget;
use crate::keywords::{self, OptionalKind};
use crate::layers::{Held, View, outside};
use crate::object::{Chosen, GameObject, ObjectKind, ObjectRef, PermanentState};
use crate::snapshot::Snapshot;
use crate::state::GameState;
use crate::zones::Zone;

/// What earlier steps of a resolution bound to a name.
pub type Bindings = BTreeMap<String, Vec<Target>>;

/// Why a value could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ResolveError {
    /// The engine does not play what the card asks for yet.
    #[error("{0} is not in the engine yet")]
    Unsupported(&'static str),
    /// The data does not make sense in the game as it is.
    #[error("{0}")]
    Invalid(String),
}

impl From<ResolveError> for crate::error::EngineError {
    fn from(error: ResolveError) -> Self {
        crate::error::EngineError::Resolution(error.to_string())
    }
}

static NO_CHOICES: Choices = Choices {
    modes: Vec::new(),
    x: None,
    targets: Vec::new(),
    division: Vec::new(),
    optional_costs: Vec::new(),
    way: None,
    face: None,
    cost_picks: None,
};
static NO_BINDINGS: Bindings = BTreeMap::new();

/// Everything a value may depend on: the game, who is acting for which object, and what the object announced and bound.
#[derive(Debug, Clone)]
pub struct Scope<'a> {
    pub(crate) state: &'a GameState,
    pub(crate) controller: PlayerId,
    pub(crate) source: ObjectRef,
    pub(crate) choices: &'a Choices,
    /// The target specifications that were announced, in the order they were asked.
    pub(crate) specs: Vec<&'a TargetSpec>,
    /// How many of them come before the ones the running effect refers to: `Target(0)` is `specs[target_base]`.
    pub(crate) target_base: usize,
    pub(crate) bindings: &'a Bindings,
    pub(crate) snapshots: Vec<&'a Snapshot>,
    /// What earlier steps of this resolution moved away, as it was: a target among them stays named.
    pub(crate) departed: &'a [Snapshot],
    pub(crate) cause: Option<&'a Cause>,
    /// The permanents as the layers have left them so far, for a value that a pass over the layers works out: the
    /// effects that apply before it are in what it reads (`layers.rs`). None reads the game as it is now.
    pub(crate) layered: Option<View<'a>>,
    /// The spell being cast and the player who casts it: the announcement reads it as a spell on the stack that this
    /// player controls, though it is still in a hand (CR 601.2a), when a cost modifier asks what it affects.
    pub(crate) casting: Option<(ObjectId, PlayerId)>,
}

/// What is known of an object: the object itself if it is still the one that was named, else how it was.
struct Seen<'a> {
    owner: PlayerId,
    controller: PlayerId,
    zone: Zone,
    kind: ObjectKind,
    chars: Cow<'a, Characteristics>,
    tapped: Option<bool>,
    /// The same card, whichever zone it has moved to since: its printed side does not change.
    card: Option<&'a GameObject>,
    /// The keywords it has: where the layers have worked them out, or as its snapshot holds them.
    keywords: Option<&'a [Keyword]>,
    /// The counters on it: its own while it is the object that was named, else those its snapshot holds.
    counters: Counted<'a>,
    /// The X it was cast with while it is a spell on the stack, else 0 (CR 202.3e).
    x: u32,
}

/// The counters of an object that was seen.
#[derive(Clone, Copy)]
enum Counted<'a> {
    /// The permanent still is the object that was named.
    Live(&'a PermanentState),
    /// How a snapshot recorded them.
    Shot(&'a [(CounterKind, u32)]),
    /// A card in exile holds the counters of its own.
    Card(&'a std::collections::BTreeMap<CounterKind, u32>),
}

impl Counted<'_> {
    /// How many counters of `kind` there are.
    fn of(self, kind: &CounterKind) -> u32 {
        match self {
            Counted::Live(permanent) => permanent.counters_of(kind),
            Counted::Shot(held) => held
                .iter()
                .find(|(found, _)| found == kind)
                .map_or(0, |(_, count)| *count),
            Counted::Card(held) => held.get(kind).copied().unwrap_or(0),
        }
    }
}

/// The X a spell on the stack was cast with; 0 for anything else (CR 202.3e).
fn stacked_x(object: &GameObject) -> u32 {
    if object.zone() == Zone::Stack && object.kind() != ObjectKind::Ability {
        object.choices().x.unwrap_or(0)
    } else {
        0
    }
}

/// Whether the predicate holds only for objects on the stack (a spell, an ability, or one of several that are).
fn on_the_stack(pred: &Pred) -> bool {
    match pred {
        Pred::Spell | Pred::Ability(_) => true,
        Pred::AnyOf(any) => !any.is_empty() && any.iter().all(on_the_stack),
        _ => false,
    }
}

/// The kind of zone a zone is.
pub(crate) const fn zone_kind(zone: Zone) -> ZoneKind {
    match zone {
        Zone::Library(_) => ZoneKind::Library,
        Zone::Hand(_) => ZoneKind::Hand,
        Zone::Battlefield => ZoneKind::Battlefield,
        Zone::Graveyard(_) => ZoneKind::Graveyard,
        Zone::Stack => ZoneKind::Stack,
        Zone::Exile => ZoneKind::Exile,
        Zone::Command(_) => ZoneKind::Command,
    }
}

impl<'a> Scope<'a> {
    /// A scope for choosing targets: nothing is announced or bound yet.
    pub fn announcing(state: &'a GameState, controller: PlayerId, source: ObjectRef) -> Self {
        Scope {
            state,
            controller,
            source,
            choices: &NO_CHOICES,
            specs: Vec::new(),
            target_base: 0,
            bindings: &NO_BINDINGS,
            snapshots: Vec::new(),
            departed: &[],
            cause: None,
            layered: None,
            casting: None,
        }
    }

    /// The same scope reading the permanents as the layers have left them so far.
    pub(crate) fn with_layered(self, view: View<'a>) -> Self {
        Scope {
            layered: Some(view),
            ..self
        }
    }

    /// A scope for judging what a trigger found: the event it is about is `cause`, and nothing is announced or bound.
    pub(crate) fn triggered(
        state: &'a GameState,
        controller: PlayerId,
        source: ObjectRef,
        cause: &'a Cause,
    ) -> Self {
        Scope {
            cause: Some(cause),
            snapshots: cause.snapshots.iter().collect(),
            ..Scope::announcing(state, controller, source)
        }
    }

    /// How `reference` was, when it is no longer the object that was named (it has changed zones since) and this
    /// scope holds a snapshot of it: last known information (CR 608.2h).
    pub(crate) fn snapshot_of(&self, reference: ObjectRef) -> Option<&'a Snapshot> {
        let live = self
            .state
            .object(reference.id)
            .is_some_and(|object| object.counter() == reference.counter);
        if live {
            return None;
        }
        self.snapshots
            .iter()
            .copied()
            .find(|shot| shot.id == reference.id && shot.counter == reference.counter)
    }

    fn seen(&self, reference: ObjectRef) -> Option<Seen<'a>> {
        let live = self.state.object(reference.id);
        if let Some(object) = live.filter(|o| o.counter() == reference.counter) {
            let tapped = object
                .permanent()
                .map(crate::object::PermanentState::tapped);
            let counters = object
                .permanent()
                .map_or(Counted::Card(&object.card_counters), Counted::Live);
            if let Some(view) = self.layered {
                return Some(match view.get(object.id()) {
                    Some(held) => Seen {
                        owner: object.owner(),
                        controller: held.controller,
                        zone: object.zone(),
                        kind: object.kind(),
                        chars: Cow::Borrowed(&held.chars),
                        tapped,
                        card: Some(object),
                        keywords: Some(&held.keywords),
                        counters,
                        x: stacked_x(object),
                    },
                    None => Seen {
                        owner: object.owner(),
                        controller: object.controller(),
                        zone: object.zone(),
                        kind: object.kind(),
                        chars: Cow::Owned(outside(self.state, object, Some(view))),
                        tapped,
                        card: Some(object),
                        keywords: None,
                        counters,
                        x: stacked_x(object),
                    },
                });
            }
            let (chars, controller) = seen_as(self.state, reference.id)?;
            let (zone, controller) = match self.casting {
                Some((spell, caster)) if spell == object.id() => (Zone::Stack, caster),
                _ => (object.zone(), controller),
            };
            return Some(Seen {
                owner: object.owner(),
                controller,
                zone,
                kind: object.kind(),
                chars: Cow::Owned(chars),
                tapped,
                card: Some(object),
                keywords: None,
                counters,
                x: stacked_x(object),
            });
        }
        let shot = self
            .snapshots
            .iter()
            .copied()
            .find(|s| s.id == reference.id && s.counter == reference.counter)?;
        Some(Seen {
            owner: shot.owner,
            controller: shot.controller,
            zone: shot.zone,
            kind: live.map_or(ObjectKind::Card, GameObject::kind),
            chars: Cow::Borrowed(&shot.characteristics),
            tapped: None,
            card: live,
            keywords: Some(shot.keywords.as_slice()),
            counters: Counted::Shot(&shot.counters),
            x: if shot.zone == Zone::Stack {
                shot.x.unwrap_or(0)
            } else {
                0
            },
        })
    }

    /// Whether the permanent `object`, which the layers have made `held` so far, satisfies every predicate of the
    /// selector: the way a static ability tests what it affects.
    pub(crate) fn selects_with(
        &self,
        selector: &Selector,
        object: &GameObject,
        held: &Held,
    ) -> Result<bool, ResolveError> {
        let seen = Seen {
            owner: object.owner(),
            controller: held.controller,
            zone: object.zone(),
            kind: object.kind(),
            chars: Cow::Borrowed(&held.chars),
            tapped: object
                .permanent()
                .map(crate::object::PermanentState::tapped),
            card: Some(object),
            keywords: Some(&held.keywords),
            counters: object
                .permanent()
                .map_or(Counted::Card(&object.card_counters), Counted::Live),
            x: stacked_x(object),
        };
        for pred in &selector.0 {
            if !self.pred(pred, object.reference(), &seen)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// The targets chosen for the specification `index` of the running effect that still count: a legal one, or one an earlier step of this
    /// resolution moved away, whose last known information the later steps read (CR 608.2h). Any other illegal one is
    /// ignored (CR 608.2b).
    fn chosen(&self, index: usize) -> Vec<Target> {
        let index = self.target_base + index;
        let (Some(set), Some(spec)) = (self.choices.targets.get(index), self.specs.get(index))
        else {
            return Vec::new();
        };
        let legal = legal_from(
            self.state,
            spec,
            self.source,
            &self.snapshots,
            self.controller,
            self.cause,
            Some(self.x_value()),
        );
        set.iter()
            .copied()
            .filter(|target| legal.contains(target) || self.departed(*target))
            .collect()
    }

    /// What an overloaded spell reads instead of the target `index`: each object or player that specification could
    /// target ("each" for "target", CR 702.96a), whatever protects it.
    fn each_target(&self, index: usize) -> Result<Vec<Target>, ResolveError> {
        let Some(spec) = self.specs.get(self.target_base + index) else {
            return Ok(Vec::new());
        };
        match &spec.what {
            Targetable::Object(selector) => self.refs(&Ref::Each(selector.clone())),
            Targetable::Player => self.refs(&Ref::EachPlayer),
            Targetable::Opponent => self.refs(&Ref::Opponents),
            Targetable::AnyTarget | Targetable::Either(_) => Err(ResolveError::Unsupported(
                "overload of a target that is not an object or a player",
            )),
        }
    }

    /// Whether an earlier step of this resolution moved the target away.
    fn departed(&self, target: Target) -> bool {
        target
            .object()
            .is_some_and(|object| self.departed.iter().any(|shot| shot.reference() == object))
    }

    /// CR 608.2b: whether targets were chosen and every one of them is illegal now, so the spell or ability does nothing.
    pub(crate) fn all_targets_illegal(&self) -> bool {
        let mut any = false;
        for (index, set) in self.choices.targets.iter().enumerate() {
            let Some(spec) = self.specs.get(index) else {
                continue;
            };
            let legal = legal_from(
                self.state,
                spec,
                self.source,
                &self.snapshots,
                self.controller,
                self.cause,
                Some(self.x_value()),
            );
            for target in set {
                any = true;
                if legal.contains(target) {
                    return false;
                }
            }
        }
        any
    }

    /// The players and objects a reference names, in the order the rules process them.
    pub fn refs(&self, reference: &Ref) -> Result<Vec<Target>, ResolveError> {
        let state = self.state;
        Ok(match reference {
            Ref::You => vec![Target::Player(self.controller)],
            Ref::Opponents => state
                .turn_order_from(state.turn().active())
                .into_iter()
                .filter(|p| *p != self.controller)
                .map(Target::Player)
                .collect(),
            Ref::EachPlayer => state
                .turn_order_from(state.turn().active())
                .into_iter()
                .map(Target::Player)
                .collect(),
            Ref::This => vec![Target::Object(self.source)],
            Ref::Target(index) if self.choices.way == Some(CastWay::Overload) => {
                self.each_target(usize::from(*index))?
            }
            Ref::Target(index) => self.chosen(usize::from(*index)),
            Ref::Triggering => self
                .cause
                .map(|cause| cause.subjects.clone())
                .ok_or_else(|| ResolveError::Invalid("there is no triggering event".into()))?,
            Ref::Attached => state
                .host_of(self.source.id)
                .and_then(|host| state.object(host))
                .map(|host| Target::Object(host.reference()))
                .into_iter()
                .collect(),
            Ref::AttachedPlayer => state
                .player_host_of(self.source.id)
                .map(Target::Player)
                .into_iter()
                .collect(),
            Ref::Monarch => state.monarch().map(Target::Player).into_iter().collect(),
            Ref::Defending => self.defending().map(Target::Player).into_iter().collect(),
            Ref::Attacked => self
                .attacked_opponents()
                .into_iter()
                .map(Target::Player)
                .collect(),
            Ref::MostLife => self.most_life().into_iter().map(Target::Player).collect(),
            Ref::Bound(name) => self
                .bindings
                .get(name)
                .cloned()
                .ok_or_else(|| ResolveError::Invalid(format!("`{name}` is not bound")))?,
            Ref::Paid => self
                .choices
                .cost_picks()
                .iter()
                .flatten()
                .filter_map(|picked| state.object(*picked))
                .map(|picked| Target::Object(picked.reference()))
                .collect(),
            Ref::ControllerOf(inner) => self.seats_of(inner, |seen| seen.controller)?,
            Ref::OwnerOf(inner) => self.seats_of(inner, |seen| seen.owner)?,
            Ref::Each(selector) => self
                .matching(selector)?
                .into_iter()
                .map(Target::Object)
                .collect(),
        })
    }

    /// The player the attacking creature of the event, else the source, attacks or attacked when it left combat.
    fn defending(&self) -> Option<PlayerId> {
        let attacker = self
            .cause
            .and_then(|cause| cause.subjects.iter().find_map(|subject| subject.object()))
            .map_or(self.source.id, |object| object.id);
        let combat = self.state.combat();
        let target = combat
            .attackers()
            .iter()
            .find(|attacking| attacking.object() == attacker)
            .map(crate::combat::Attacker::target)
            .or_else(|| combat.left_attacking(attacker))?;
        crate::combat::defending_player(self.state, target)
    }

    /// The opponents of the controller their creatures attacked this combat, in turn order.
    fn attacked_opponents(&self) -> Vec<PlayerId> {
        let combat = self.state.combat();
        let attacked: Vec<PlayerId> = combat
            .attackers()
            .iter()
            .map(crate::combat::Attacker::target)
            .chain(combat.left.iter().map(|(_, target)| *target))
            .filter_map(|target| crate::combat::defending_player(self.state, target))
            .collect();
        self.state
            .turn_order_from(self.state.turn().active())
            .into_iter()
            .filter(|player| *player != self.controller && attacked.contains(player))
            .collect()
    }

    /// The players with the most life among those still in the game.
    fn most_life(&self) -> Vec<PlayerId> {
        let life = |player: PlayerId| self.state.player(player).map(crate::player::Player::life);
        let alive: Vec<PlayerId> = self.state.alive_players().collect();
        let most = alive.iter().filter_map(|player| life(*player)).max();
        alive
            .into_iter()
            .filter(|player| life(*player) == most)
            .collect()
    }

    fn seats_of(
        &self,
        inner: &Ref,
        seat: impl Fn(&Seen<'_>) -> PlayerId,
    ) -> Result<Vec<Target>, ResolveError> {
        let mut seats = Vec::new();
        for object in self.objects(inner)? {
            if let Some(seen) = self.seen(object) {
                let player = Target::Player(seat(&seen));
                if !seats.contains(&player) {
                    seats.push(player);
                }
            }
        }
        Ok(seats)
    }

    /// The players a reference names (objects among them are ignored).
    pub fn players(&self, reference: &Ref) -> Result<Vec<PlayerId>, ResolveError> {
        Ok(self
            .refs(reference)?
            .into_iter()
            .filter_map(Target::player)
            .collect())
    }

    /// The objects a reference names (players among them are ignored).
    pub fn objects(&self, reference: &Ref) -> Result<Vec<ObjectRef>, ResolveError> {
        Ok(self
            .refs(reference)?
            .into_iter()
            .filter_map(Target::object)
            .collect())
    }

    /// The objects a reference names that are still the objects it named, each once: one that has changed zones since
    /// is a new object (CR 400.7) and is passed over, so is a name that repeats.
    pub(crate) fn live_objects(
        &self,
        reference: &Ref,
    ) -> Result<Vec<&'a GameObject>, ResolveError> {
        let state = self.state;
        let mut found: Vec<&'a GameObject> = Vec::new();
        for named in self.objects(reference)? {
            if let Some(object) = state
                .object(named.id)
                .filter(|object| object.counter() == named.counter)
                && !found.iter().any(|seen| seen.id() == object.id())
            {
                found.push(object);
            }
        }
        Ok(found)
    }

    /// The zones a selector looks in: the one it names, the stack for spells, else the battlefield.
    fn zones_of(&self, selector: &Selector) -> Vec<Zone> {
        let seats = || (0..self.state.players().len()).map(PlayerId::from_index);
        let named = selector.0.iter().find_map(|pred| match pred {
            Pred::In(kind) => Some(*kind),
            _ => None,
        });
        match named {
            Some(ZoneKind::Hand) => seats().map(Zone::Hand).collect(),
            Some(ZoneKind::Library) => seats().map(Zone::Library).collect(),
            Some(ZoneKind::Graveyard) => seats().map(Zone::Graveyard).collect(),
            Some(ZoneKind::Stack) => vec![Zone::Stack],
            Some(ZoneKind::Exile) => vec![Zone::Exile],
            Some(ZoneKind::Command) => seats().map(Zone::Command).collect(),
            None if selector.0.iter().any(on_the_stack) => vec![Zone::Stack],
            Some(ZoneKind::Battlefield) | None => vec![Zone::Battlefield],
        }
    }

    /// Every object the selector matches, in id order.
    pub fn matching(&self, selector: &Selector) -> Result<Vec<ObjectRef>, ResolveError> {
        let mut found = Vec::new();
        for zone in self.zones_of(selector) {
            for id in self.state.zone(zone) {
                let Some(object) = self.state.object(*id) else {
                    continue;
                };
                let reference = object.reference();
                if self.selects(selector, reference)? {
                    found.push(reference);
                }
            }
        }
        found.sort_unstable();
        Ok(found)
    }

    /// Whether the object satisfies every predicate of the selector.
    pub fn selects(&self, selector: &Selector, reference: ObjectRef) -> Result<bool, ResolveError> {
        let Some(seen) = self.seen(reference) else {
            return Ok(false);
        };
        for pred in &selector.0 {
            if !self.pred(pred, reference, &seen)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether the object shares a subtype (its printed ones included) with one of the objects the reference names.
    fn shares_subtype(&self, named: &Ref, seen: &Seen<'_>) -> Result<bool, ResolveError> {
        for other in self.objects(named)? {
            let Some(theirs) = self.seen(other) else {
                continue;
            };
            let printed = theirs.card.map(|c| c.card().subtypes.as_slice());
            let shares = theirs
                .chars
                .subtypes
                .iter()
                .chain(printed.unwrap_or_default())
                .any(|subtype| {
                    seen.chars.has_subtype(&[], subtype)
                        || seen
                            .card
                            .is_some_and(|c| seen.chars.has_subtype(&c.card().subtypes, subtype))
                });
            if shares {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn pred(
        &self,
        pred: &Pred,
        reference: ObjectRef,
        seen: &Seen<'_>,
    ) -> Result<bool, ResolveError> {
        let state = self.state;
        Ok(match pred {
            Pred::This => reference.id == self.source.id,
            Pred::Other => reference.id != self.source.id,
            Pred::Is(named) => self.objects(named)?.contains(&reference),
            Pred::SharesSubtype(named) => self.shares_subtype(named, seen)?,
            Pred::Type(card_type) => seen.chars.types.contains(*card_type),
            Pred::Supertype(supertype) => seen
                .card
                .is_some_and(|c| c.card().supertypes.contains(supertype)),
            Pred::Subtype(subtype) => {
                seen.chars.has_subtype(&[], subtype)
                    || seen
                        .card
                        .is_some_and(|c| seen.chars.has_subtype(&c.card().subtypes, subtype))
            }
            Pred::Color(color) => seen.chars.colors.contains(*color),
            Pred::Colorless => seen.chars.colors.is_empty(),
            Pred::Token => seen.kind == ObjectKind::Token,
            Pred::Commander => state.is_commander(reference.id),
            Pred::Spell => seen.zone == Zone::Stack && seen.kind != ObjectKind::Ability,
            Pred::Ability(kind) => {
                seen.zone == Zone::Stack
                    && seen
                        .card
                        .and_then(GameObject::ability)
                        .is_some_and(|ability| {
                            matches!(
                                (kind, ability.rules()),
                                (AbilityKind::Triggered, Ability::Triggered { .. })
                                    | (
                                        AbilityKind::Activated,
                                        Ability::Activated { .. } | Ability::Loyalty { .. }
                                    )
                            )
                        })
            }
            Pred::Tapped => seen.tapped == Some(true),
            Pred::Untapped => seen.tapped == Some(false),
            Pred::Monstrous => matches!(seen.counters, Counted::Live(held) if held.monstrous()),
            Pred::Attacking => state
                .combat()
                .attackers()
                .iter()
                .any(|a| a.object() == reference.id),
            Pred::AttackingPlayer(who) => {
                let attacked = self.players(who)?;
                state.combat().attackers().iter().any(|a| {
                    a.object() == reference.id
                        && matches!(a.target(), AttackTarget::Player(player) if attacked.contains(&player))
                })
            }
            Pred::Blocking => state
                .combat()
                .attackers()
                .iter()
                .any(|a| a.blockers().contains(&reference.id)),
            Pred::Unblocked => state.combat().is_unblocked(state, reference.id),
            Pred::ControlledBy(who) => self.players(who)?.contains(&seen.controller),
            Pred::OwnedBy(who) => self.players(who)?.contains(&seen.owner),
            Pred::In(kind) => zone_kind(seen.zone) == *kind,
            Pred::CastFrom(kind) => self.cast_from(seen, *kind),
            // An object that has changed zones since it was named has the keywords its snapshot holds (CR 608.2h).
            Pred::HasKeyword(keyword) => match seen.keywords {
                Some(held) => held.contains(keyword),
                None => seen
                    .card
                    .is_some_and(|card| keywords::has_keyword(state, card.id(), keyword)),
            },
            Pred::HasCounter(kind) => seen.counters.of(kind) > 0,
            Pred::Named(name) => seen.card.is_some_and(|c| c.card().name == *name),
            Pred::ChosenColor => {
                matches!(self.entered_choice(), Some(Chosen::Color(color)) if seen.chars.colors.contains(*color))
            }
            Pred::ChosenSubtype => {
                matches!(self.entered_choice(), Some(Chosen::CreatureType(subtype))
                    if seen.chars.has_subtype(&[], subtype)
                        || seen.card.is_some_and(|c| seen.chars.has_subtype(&c.card().subtypes, subtype)))
            }
            Pred::Compare { stat, op, to } => match Self::stat(seen, *stat) {
                Some(value) => op.holds(value, self.amount(to)?),
                None => false,
            },
            Pred::Not(inner) => !self.pred(inner, reference, seen)?,
            Pred::AnyOf(any) => {
                let mut holds = false;
                for inner in any {
                    if self.pred(inner, reference, seen)? {
                        holds = true;
                        break;
                    }
                }
                holds
            }
        })
    }

    /// What the source of the scope chose as it entered, if it is on the battlefield and asked for a choice.
    fn entered_choice(&self) -> Option<&'a Chosen> {
        self.state
            .object(self.source.id)
            .and_then(|object| object.permanent())
            .and_then(|permanent| permanent.chosen())
    }

    fn stat(seen: &Seen<'_>, stat: StatKind) -> Option<i64> {
        match stat {
            StatKind::Power => seen.chars.power.map(i64::from),
            StatKind::Toughness => seen.chars.toughness.map(i64::from),
            StatKind::Colors => i64::try_from(seen.chars.colors.iter().count()).ok(),
            // A spell on the stack counts the X it was cast with for each {X} of its cost (CR 202.3e).
            StatKind::ManaValue => seen
                .card
                .map(|c| i64::from(c.mana_value() + u32::from(c.card().x_in_cost) * seen.x)),
            // The loyalty of a planeswalker on the battlefield is its loyalty counters, as they were if it has left;
            // in any other zone it is the number printed on the card (CR 306.5a, 306.5c). Nothing else has loyalty.
            StatKind::Loyalty => seen.chars.types.contains(CardType::Planeswalker).then(|| {
                if seen.zone == Zone::Battlefield {
                    i64::from(seen.counters.of(&CounterKind::Loyalty))
                } else {
                    seen.card
                        .and_then(|card| card.card().loyalty)
                        .map_or(0, i64::from)
                }
            }),
        }
    }

    /// The number an amount stands for.
    pub fn amount(&self, amount: &Amount) -> Result<i64, ResolveError> {
        self.amount_of(amount, None)
    }

    /// The number an amount stands for as one recipient of the running effect reads it (`share`, see `share.rs`).
    pub(super) fn amount_of(
        &self,
        amount: &Amount,
        share: Option<Share>,
    ) -> Result<i64, ResolveError> {
        Ok(match amount {
            Amount::Fixed(n) => i64::from(*n),
            Amount::X => i64::from(self.x_value()),
            Amount::Count(selector) => i64::try_from(self.matching(selector)?.len())
                .map_err(|_| ResolveError::Invalid("too many objects".into()))?,
            Amount::Stat { of, stat } => match self.objects(of)?.first() {
                Some(object) => self
                    .seen(*object)
                    .and_then(|seen| Self::stat(&seen, *stat))
                    .unwrap_or(0),
                None => 0,
            },
            Amount::Life(who) => self
                .players(who)?
                .first()
                .and_then(|p| self.state.player(*p))
                .map_or(0, |p| i64::from(p.life())),
            Amount::Counters { on, counter } => {
                let mut total = 0i64;
                for named in self.refs(on)? {
                    total += match named {
                        Target::Player(player) => self
                            .state
                            .player(player)
                            .map_or(0, |held| i64::from(held.counters_of(counter))),
                        Target::Object(object) => self
                            .seen(object)
                            .map_or(0, |seen| i64::from(seen.counters.of(counter))),
                    };
                }
                total
            }
            Amount::Sum(parts) => {
                let mut sum = 0i64;
                for part in parts {
                    sum += self.amount_of(part, share)?;
                }
                sum
            }
            Amount::Times { by, of } => i64::from(*by) * self.amount_of(of, share)?,
            Amount::Divided => {
                let share = share.ok_or(ResolveError::Unsupported(OUTSIDE_A_DIVISION))?;
                self.divided_share(share.recipient)
            }
            // "Divided evenly, rounded down": the whole is split among the recipients still there, whatever
            // was announced.
            Amount::EvenShare(whole) => {
                let share = share.ok_or(ResolveError::Unsupported(OUTSIDE_A_DIVISION))?;
                let whole = self.amount_of(whole, Some(share))?.max(0);
                match i64::try_from(share.among) {
                    Ok(among) if among > 0 => whole / among,
                    _ => 0,
                }
            }
            Amount::TargetCount(index) => i64::try_from(self.chosen(usize::from(*index)).len())
                .map_err(|_| ResolveError::Invalid("too many targets".into()))?,
            Amount::Devotion(_) => return Err(ResolveError::Unsupported("devotion")),
            Amount::Greatest { of, stat } => self.stats(of, *stat)?.into_iter().max().unwrap_or(0),
            Amount::Total { of, stat } => self.stats(of, *stat)?.into_iter().sum(),
            Amount::CardTypes(of) => self.card_types_among(of)?,
            Amount::Players(who) => i64::try_from(self.players(who)?.len())
                .map_err(|_| ResolveError::Invalid("too many players".into()))?,
            Amount::LowestLife(who) => self.lives(who)?.min().unwrap_or(i64::from(i32::MAX)),
            Amount::HighestLife(who) => self.lives(who)?.max().unwrap_or(i64::from(i32::MIN)),
            Amount::Kicks => i64::try_from(self.payments(OptionalKind::Kicker))
                .map_err(|_| ResolveError::Invalid("too many kicks".into()))?,
            Amount::Triggering => self
                .cause
                .map(|cause| i64::from(cause.amount))
                .ok_or_else(|| ResolveError::Invalid("there is no triggering event".into()))?,
        })
    }

    /// The value of X this scope reads (CR 107.3): what the spell or ability announced or, for a spell that is
    /// entering the battlefield, the X it was cast with (CR 107.3m); 0 when no X was chosen.
    pub(crate) fn x_value(&self) -> u32 {
        self.announced_x().unwrap_or(0)
    }

    /// [`Scope::x_value`] when an X was chosen, else nothing.
    pub(crate) fn announced_x(&self) -> Option<u32> {
        self.choices.x.or_else(|| {
            self.state
                .object(self.source.id)
                .filter(|object| {
                    object.counter() == self.source.counter && object.zone() == Zone::Stack
                })
                .and_then(GameObject::announced_x)
        })
    }

    /// A characteristic of each object the selector matches that has one.
    fn stats(&self, of: &Selector, stat: StatKind) -> Result<Vec<i64>, ResolveError> {
        Ok(self
            .matching(of)?
            .into_iter()
            .filter_map(|object| self.seen(object).and_then(|seen| Self::stat(&seen, stat)))
            .collect())
    }

    /// How many of the card types there are among the objects the selector matches, each counted once.
    fn card_types_among(&self, of: &Selector) -> Result<i64, ResolveError> {
        const TYPES: [CardType; 9] = [
            CardType::Artifact,
            CardType::Battle,
            CardType::Creature,
            CardType::Enchantment,
            CardType::Instant,
            CardType::Kindred,
            CardType::Land,
            CardType::Planeswalker,
            CardType::Sorcery,
        ];
        let seen: Vec<Seen<'a>> = self
            .matching(of)?
            .into_iter()
            .filter_map(|object| self.seen(object))
            .collect();
        let among = TYPES
            .iter()
            .filter(|card_type| seen.iter().any(|one| one.chars.types.contains(**card_type)))
            .count();
        i64::try_from(among).map_err(|_| ResolveError::Invalid("too many card types".into()))
    }

    /// The life totals of the players a reference names.
    fn lives(&self, who: &Ref) -> Result<impl Iterator<Item = i64>, ResolveError> {
        let players = self.players(who)?;
        Ok(players
            .into_iter()
            .filter_map(|player| self.state.player(player))
            .map(|player| i64::from(player.life())))
    }

    /// The optional costs paid as the spell was cast, by index, a repeatable cost once for each payment: those the
    /// spell or ability announced, else those of the spell its source is or came from while that is still the object
    /// named (the spell on the stack, the permanent it became).
    fn paid_costs(&self) -> &[usize] {
        if !self.choices.optional_costs.is_empty() {
            return &self.choices.optional_costs;
        }
        self.state
            .object(self.source.id)
            .filter(|object| object.counter() == self.source.counter)
            .map_or(&[], GameObject::paid_costs)
    }

    /// How many times the optional costs of `kind` were paid (`paid_costs`), read against the costs the card prints.
    fn payments(&self, kind: OptionalKind) -> usize {
        self.state
            .object(self.source.id)
            .map_or(0, |source| payments(source.card(), self.paid_costs(), kind))
    }

    /// The alternative cost the spell was cast for: what the spell or ability announced, else what the spell its source is
    /// or came from was cast for while that is still the object named.
    fn cast_way(&self) -> Option<CastWay> {
        self.choices.way.or_else(|| {
            self.state
                .object(self.source.id)
                .filter(|object| object.counter() == self.source.counter)
                .and_then(GameObject::cast_way)
                .or_else(|| {
                    self.snapshot_of(self.source)
                        .and_then(|snapshot| snapshot.way)
                })
        })
    }

    /// Whether `seen` is a spell cast from a zone of this kind: a spell being announced still lies there, one on the
    /// stack remembers a hand or a graveyard.
    fn cast_from(&self, seen: &Seen<'_>, kind: ZoneKind) -> bool {
        let Some(spell) = seen
            .card
            .filter(|_| seen.zone == Zone::Stack && seen.kind != ObjectKind::Ability)
        else {
            return false;
        };
        if self.casting.is_some_and(|(cast, _)| cast == spell.id()) {
            return zone_kind(spell.zone()) == kind;
        }
        match kind {
            ZoneKind::Hand => spell.cast_from_hand(),
            ZoneKind::Graveyard => spell.cast_from_graveyard(),
            _ => false,
        }
    }

    /// Whether a condition is true now.
    pub fn holds(&self, condition: &Condition) -> Result<bool, ResolveError> {
        Ok(match condition {
            Condition::Not(inner) => !self.holds(inner)?,
            Condition::All(all) => {
                let mut holds = true;
                for inner in all {
                    if !self.holds(inner)? {
                        holds = false;
                        break;
                    }
                }
                holds
            }
            Condition::Any(any) => {
                let mut holds = false;
                for inner in any {
                    if self.holds(inner)? {
                        holds = true;
                        break;
                    }
                }
                holds
            }
            Condition::Compare { left, op, right } => {
                op.holds(self.amount(left)?, self.amount(right)?)
            }
            Condition::Matches { subject, selector } => {
                let mut holds = true;
                for object in self.objects(subject)? {
                    if !self.selects(selector, object)? {
                        holds = false;
                        break;
                    }
                }
                holds
            }
            Condition::Kicked => self.payments(OptionalKind::Kicker) > 0,
            Condition::AdditionalCostPaid => self.payments(OptionalKind::Additional) > 0,
            Condition::CastWith(way) => self.cast_way() == Some(*way),
            Condition::Performed => return Err(ResolveError::Unsupported("\"if you do\"")),
            Condition::Monarch(who) => {
                let players = self.players(who)?;
                // One player at most is the monarch, so "an opponent is the monarch" holds when any of them is.
                players
                    .iter()
                    .any(|player| self.state.monarch() == Some(*player))
            }
            Condition::CitysBlessing(who) => self
                .players(who)?
                .iter()
                .any(|player| self.state.has_citys_blessing(*player)),
            Condition::YourTurn => self.state.turn().active() == self.controller,
            Condition::Level { of, op, level } => {
                let named = self.objects(of)?;
                !named.is_empty()
                    && named.iter().all(|named| {
                        self.state
                            .object(named.id)
                            .filter(|object| object.counter() == named.counter)
                            .and_then(GameObject::permanent)
                            .is_some_and(|permanent| {
                                op.holds(i64::from(permanent.level()), i64::from(*level))
                            })
                    })
            }
            Condition::NewlyControlled(what) => {
                let named = self.objects(what)?;
                !named.is_empty()
                    && named.iter().all(|named| {
                        self.state
                            .object(named.id)
                            .filter(|object| object.counter() == named.counter)
                            .and_then(GameObject::permanent)
                            .is_some_and(|permanent| permanent.is_new_to(self.controller))
                            && controller_of(self.state, named.id) == Some(self.controller)
                    })
            }
        })
    }

    /// Whether the object is currently a creature, planeswalker or battle: what damage can be dealt to (CR 120.1).
    pub(crate) fn can_be_dealt_damage(&self, object: ObjectRef) -> bool {
        self.state
            .object(object.id)
            .filter(|o| o.counter() == object.counter && o.zone() == Zone::Battlefield)
            .and_then(|o| characteristics(self.state, o.id()))
            .is_some_and(|chars| {
                [CardType::Creature, CardType::Planeswalker, CardType::Battle]
                    .into_iter()
                    .any(|ty| chars.types.contains(ty))
            })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: references, selectors, amounts and conditions read the game as the rules say, an object that changed zones is read as it was when a snapshot of it exists, a target that an earlier step of the resolution moved away stays named while any other illegal target is ignored, and what the engine cannot evaluate yet refuses by name.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::refs::{Bounds, Targetable};
    use mtg_core::card::{CardType, Supertype};
    use mtg_core::ids::ObjectId;
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;
    use crate::counters::CountersChanged;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::state;
    use crate::fold::apply_event;
    use crate::snapshot::snapshot;

    fn bear() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Bear")
                .with_type(CardType::Creature)
                .with_subtype("Bear")
                .with_mana_cost(ManaCost::parse("{1}{G}").unwrap())
                .with_power_toughness(2, 2),
        )
    }

    fn ogre() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Ogre")
                .with_type(CardType::Creature)
                .with_subtype("Ogre")
                .with_mana_cost(ManaCost::parse("{3}{R}").unwrap())
                .with_power_toughness(3, 4),
        )
    }

    fn relic() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Relic")
                .with_type(CardType::Artifact)
                .with_supertype(Supertype::Legendary)
                .with_mana_cost(ManaCost::parse("{2}").unwrap()),
        )
    }

    /// Seat 0 controls a Bear and a Relic, seat 1 an Ogre and a Bear; seat 0 has a Bear in hand.
    fn game() -> GameState {
        state(
            vec![vec![bear()], vec![]],
            vec![vec![bear(), relic()], vec![ogre(), bear()]],
        )
    }

    fn named(game: &GameState, name: &str, controller: usize) -> ObjectRef {
        game.zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| game.object(*id))
            .find(|o| o.card().name == name && o.controller() == PlayerId::from_index(controller))
            .map(GameObject::reference)
            .unwrap()
    }

    fn scope(game: &GameState) -> Scope<'_> {
        Scope::announcing(game, PlayerId(0), named(game, "Relic", 0))
    }

    fn selector(json: &str) -> Selector {
        serde_json::from_str(json).unwrap()
    }

    /// What a selector matches on the battlefield, as names with the seat that controls each.
    fn matched(game: &GameState, json: &str) -> Vec<String> {
        scope(game)
            .matching(&selector(json))
            .unwrap()
            .into_iter()
            .map(|r| {
                let object = game.object(r.id).unwrap();
                format!("{} {}", object.card().name, object.controller().get())
            })
            .collect()
    }

    #[rstest]
    #[case::everything("[]", &["Bear 0", "Relic 0", "Ogre 1", "Bear 1"])]
    #[case::creatures(r#"[{"Type":"Creature"}]"#, &["Bear 0", "Ogre 1", "Bear 1"])]
    #[case::artifacts(r#"[{"Type":"Artifact"}]"#, &["Relic 0"])]
    #[case::controlled_by_you(r#"[{"ControlledBy":"You"}]"#, &["Bear 0", "Relic 0"])]
    #[case::controlled_by_opponents(r#"[{"ControlledBy":"Opponents"}]"#, &["Ogre 1", "Bear 1"])]
    #[case::owned_by_you_and_a_creature(r#"[{"OwnedBy":"You"},{"Type":"Creature"}]"#, &["Bear 0"])]
    #[case::other_than_the_source(r#"["Other"]"#, &["Bear 0", "Ogre 1", "Bear 1"])]
    #[case::only_the_source(r#"["This"]"#, &["Relic 0"])]
    #[case::a_subtype(r#"[{"Subtype":"Ogre"}]"#, &["Ogre 1"])]
    #[case::a_supertype(r#"[{"Supertype":"Legendary"}]"#, &["Relic 0"])]
    #[case::a_color(r#"[{"Color":"Green"}]"#, &["Bear 0", "Bear 1"])]
    #[case::colorless(r#"["Colorless"]"#, &["Relic 0"])]
    #[case::a_name(r#"[{"Named":"Ogre"}]"#, &["Ogre 1"])]
    #[case::power(r#"[{"Compare":{"stat":"Power","op":"Ge","to":3}}]"#, &["Ogre 1"])]
    #[case::mana_value(r#"[{"Compare":{"stat":"ManaValue","op":"Eq","to":2}}]"#, &["Bear 0", "Relic 0", "Bear 1"])]
    #[case::not(r#"[{"Not":{"Type":"Creature"}}]"#, &["Relic 0"])]
    #[case::any_of(r#"[{"AnyOf":[{"Subtype":"Ogre"},{"Type":"Artifact"}]}]"#, &["Relic 0", "Ogre 1"])]
    #[case::untapped(r#"["Untapped"]"#, &["Bear 0", "Relic 0", "Ogre 1", "Bear 1"])]
    #[case::tapped_is_none_here(r#"["Tapped"]"#, &[])]
    #[case::tokens_do_not_exist_yet(r#"["Token"]"#, &[])]
    #[case::attacking_is_none_out_of_combat(r#"["Attacking"]"#, &[])]
    #[case::blocking_is_none_out_of_combat(r#"["Blocking"]"#, &[])]
    #[case::a_keyword_nothing_has(r#"[{"HasKeyword":"Flying"}]"#, &[])]
    #[case::the_hand_instead(r#"[{"In":"Hand"}]"#, &["Bear 0"])]
    #[case::the_graveyard_is_empty(r#"[{"In":"Graveyard"}]"#, &[])]
    #[case::spells_are_on_the_stack(r#"["Spell"]"#, &[])]
    fn a_selector_matches_the_objects_that_satisfy_every_predicate(
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(matched(&game(), json), expected);
    }

    fn names(game: &GameState, targets: &[Target]) -> Vec<String> {
        targets
            .iter()
            .map(|t| match t {
                Target::Player(p) => format!("player {}", p.get()),
                Target::Object(o) => {
                    let object = game.object(o.id).unwrap();
                    format!("{} {}", object.card().name, object.controller().get())
                }
            })
            .collect()
    }

    #[rstest]
    #[case::you(r#""You""#, &["player 0"])]
    #[case::opponents(r#""Opponents""#, &["player 1"])]
    #[case::each_player(r#""EachPlayer""#, &["player 0", "player 1"])]
    #[case::this(r#""This""#, &["Relic 0"])]
    #[case::each(r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}"#, &["Ogre 1", "Bear 1"])]
    #[case::controller_of(r#"{"ControllerOf":{"Each":[{"Named":"Ogre"}]}}"#, &["player 1"])]
    #[case::owner_of_several_once(r#"{"OwnerOf":{"Each":[{"Type":"Creature"}]}}"#, &["player 0", "player 1"])]
    #[case::a_source_with_no_host_names_nothing(r#""Attached""#, &[])]
    fn a_reference_names_its_players_and_objects(#[case] json: &str, #[case] expected: &[&str]) {
        let game = game();
        let reference: Ref = serde_json::from_str(json).unwrap();
        let found = scope(&game).refs(&reference).unwrap();
        assert_eq!(names(&game, &found), expected);
    }

    #[test]
    fn attached_names_the_host_of_the_source_while_it_is_the_object_it_was_attached_to() {
        let mut setup = crate::fixtures::setup(
            vec![vec![], vec![]],
            vec![vec![bear(), relic()], vec![ogre()]],
        );
        setup.players[0].battlefield[1].attached_to = Some((1, 0));
        let mut game = GameState::from_setup(&setup).unwrap();
        let relic = named(&game, "Relic", 0);
        let host = |game: &GameState| {
            let found = Scope::announcing(game, PlayerId(0), relic)
                .refs(&Ref::Attached)
                .unwrap();
            names(game, &found)
        };
        assert_eq!(host(&game), ["Ogre 1"]);
        // A host that leaves is a new object when it comes back, and the Relic is not attached to it.
        let ogre = named(&game, "Ogre", 1).id;
        for to in [Zone::Graveyard(PlayerId(1)), Zone::Battlefield] {
            let from = game.object(ogre).unwrap().zone();
            apply_event(
                &mut game,
                &Event::ZoneChanged {
                    object: ogre,
                    from,
                    to,
                    cause: crate::events::ZoneCause::Moved,
                },
            )
            .unwrap();
        }
        assert_eq!(host(&game), Vec::<String>::new());
    }

    #[test]
    fn opponents_and_each_player_follow_turn_order_from_the_active_player() {
        let game = state(vec![vec![], vec![], vec![]], vec![vec![], vec![], vec![]]);
        let scope = Scope::announcing(
            &game,
            PlayerId(1),
            ObjectRef {
                id: ObjectId(0),
                counter: 0,
            },
        );
        let seats = |reference: &str| -> Vec<u64> {
            scope
                .players(&serde_json::from_str(reference).unwrap())
                .unwrap()
                .iter()
                .map(|p| p.get())
                .collect()
        };
        // Seat 0 is the active player, so it comes first even for a spell of seat 1's.
        assert_eq!(seats(r#""EachPlayer""#), [0, 1, 2]);
        assert_eq!(seats(r#""Opponents""#), [0, 2]);
        assert_eq!(seats(r#""You""#), [1]);
    }

    #[rstest]
    #[case::fixed("3", 3)]
    #[case::negative("-2", -2)]
    #[case::count(r#"{"Count":[{"Type":"Creature"}]}"#, 3)]
    #[case::count_yours(r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#, 1)]
    #[case::power_of_an_object(r#"{"Stat":{"of":{"Each":[{"Named":"Ogre"}]},"stat":"Power"}}"#, 3)]
    #[case::toughness_of_the_source_which_has_none(
        r#"{"Stat":{"of":"This","stat":"Toughness"}}"#,
        0
    )]
    #[case::mana_value(r#"{"Stat":{"of":{"Each":[{"Named":"Ogre"}]},"stat":"ManaValue"}}"#, 4)]
    #[case::life(r#"{"Life":"You"}"#, 20)]
    #[case::players_other_than_you(r#"{"Players":"Opponents"}"#, 1)]
    #[case::lowest_life_of_everyone(r#"{"LowestLife":"EachPlayer"}"#, 20)]
    #[case::highest_life_of_the_opponents(r#"{"HighestLife":"Opponents"}"#, 20)]
    #[case::lowest_life_of_no_one(r#"{"LowestLife":{"Each":[{"Named":"Nobody"}]}}"#, 2_147_483_647)]
    #[case::highest_life_of_no_one(r#"{"HighestLife":{"Each":[{"Named":"Nobody"}]}}"#, -2_147_483_648)]
    #[case::sum(r#"{"Sum":[2,{"Count":[{"Type":"Artifact"}]},-1]}"#, 2)]
    #[case::greatest_power(r#"{"Greatest":{"of":[{"Type":"Creature"}],"stat":"Power"}}"#, 3)]
    #[case::greatest_of_yours(
        r#"{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"ManaValue"}}"#,
        2
    )]
    #[case::greatest_toughness_of_everything(r#"{"Greatest":{"of":[],"stat":"Toughness"}}"#, 4)]
    #[case::greatest_of_the_hand(r#"{"Greatest":{"of":[{"In":"Hand"}],"stat":"ManaValue"}}"#, 2)]
    #[case::greatest_of_nothing(r#"{"Greatest":{"of":[{"Named":"Nobody"}],"stat":"Power"}}"#, 0)]
    #[case::greatest_of_what_has_no_power(
        r#"{"Greatest":{"of":[{"Type":"Artifact"}],"stat":"Power"}}"#,
        0
    )]
    #[case::total_power(r#"{"Total":{"of":[{"Type":"Creature"}],"stat":"Power"}}"#, 7)]
    #[case::total_toughness_of_yours(
        r#"{"Total":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Toughness"}}"#,
        2
    )]
    #[case::total_mana_value_of_everything(r#"{"Total":{"of":[],"stat":"ManaValue"}}"#, 10)]
    #[case::total_of_nothing(r#"{"Total":{"of":[{"Named":"Nobody"}],"stat":"Power"}}"#, 0)]
    #[case::card_types_of_everything(r#"{"CardTypes":[]}"#, 2)]
    #[case::card_types_of_creatures(r#"{"CardTypes":[{"Type":"Creature"}]}"#, 1)]
    #[case::card_types_of_yours(r#"{"CardTypes":[{"ControlledBy":"You"}]}"#, 2)]
    #[case::card_types_of_theirs(r#"{"CardTypes":[{"ControlledBy":"Opponents"}]}"#, 1)]
    #[case::card_types_of_the_hand(r#"{"CardTypes":[{"In":"Hand"}]}"#, 1)]
    #[case::card_types_of_nothing(r#"{"CardTypes":[{"Named":"Nobody"}]}"#, 0)]
    #[case::no_targets_chosen(r#"{"TargetCount":0}"#, 0)]
    #[case::twice_the_creatures(r#"{"Times":{"by":2,"of":{"Count":[{"Type":"Creature"}]}}}"#, 6)]
    #[case::minus_the_creatures_you_control(
        r#"{"Times":{"by":-1,"of":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#,
        -1
    )]
    #[case::times_zero_things(r#"{"Times":{"by":3,"of":{"Count":[{"Type":"Land"}]}}}"#, 0)]
    #[case::times_the_opponents(r#"{"Times":{"by":2,"of":{"Players":"Opponents"}}}"#, 2)]
    #[case::cards_in_your_hand(r#"{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}"#, 1)]
    #[case::cards_in_your_graveyard(r#"{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"}]}"#, 0)]
    #[case::each_opponents_creatures(
        r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}"#,
        2
    )]
    fn an_amount_is_the_number_it_stands_for(#[case] json: &str, #[case] expected: i64) {
        let game = game();
        let amount: Amount = serde_json::from_str(json).unwrap();
        assert_eq!(scope(&game).amount(&amount), Ok(expected));
    }

    #[test]
    fn a_count_is_read_from_the_state_the_earlier_steps_left() {
        let mut game = game();
        let creatures: Amount = serde_json::from_str(r#"{"Count":[{"Type":"Creature"}]}"#).unwrap();
        let twice: Amount =
            serde_json::from_str(r#"{"Times":{"by":2,"of":{"Count":[{"Type":"Creature"}]}}}"#)
                .unwrap();
        assert_eq!(scope(&game).amount(&creatures), Ok(3));
        let ogre = named(&game, "Ogre", 1);
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: ogre.id,
                from: Zone::Battlefield,
                to: Zone::Graveyard(PlayerId(1)),
                cause: ZoneCause::Destroyed,
            },
        )
        .unwrap();
        assert_eq!(scope(&game).amount(&creatures), Ok(2), "the Ogre died");
        assert_eq!(scope(&game).amount(&twice), Ok(4));
        let yours = |json: &str| scope(&game).amount(&serde_json::from_str(json).unwrap());
        assert_eq!(
            yours(r#"{"Count":[{"In":"Graveyard"},{"OwnedBy":"Opponents"}]}"#),
            Ok(1),
            "the Ogre is in its owner's graveyard"
        );
    }

    #[rstest]
    #[case::two_seats(2, 1)]
    #[case::three_seats(3, 2)]
    #[case::four_seats(4, 3)]
    fn for_each_opponent_counts_the_seats_other_than_the_controller(
        #[case] seats: usize,
        #[case] opponents: i64,
    ) {
        let game = state(
            vec![vec![]; seats],
            (0..seats).map(|_| vec![bear()]).collect(),
        );
        let scope = Scope::announcing(&game, PlayerId(0), named(&game, "Bear", 0));
        let count = |json: &str| scope.amount(&serde_json::from_str(json).unwrap());
        assert_eq!(count(r#"{"Players":"Opponents"}"#), Ok(opponents));
        assert_eq!(
            count(r#"{"Times":{"by":-3,"of":{"Players":"Opponents"}}}"#),
            Ok(-3 * opponents)
        );
        let seats = i64::try_from(seats).unwrap();
        assert_eq!(count(r#"{"Count":[{"Type":"Creature"}]}"#), Ok(seats));
        assert_eq!(
            count(r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#),
            Ok(1)
        );
        assert_eq!(
            count(r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}"#),
            Ok(opponents)
        );
    }

    #[rstest]
    #[case::devotion(r#"{"Devotion":["Red"]}"#, "devotion")]
    #[case::divided(
        r#""Divided""#,
        "a divided amount outside the damage or counters it divides"
    )]
    #[case::even_share(
        r#"{"EvenShare":4}"#,
        "a divided amount outside the damage or counters it divides"
    )]
    fn what_the_engine_cannot_evaluate_yet_says_what_it_lacks(
        #[case] json: &str,
        #[case] lacks: &'static str,
    ) {
        let game = game();
        let amount: Amount = serde_json::from_str(json).unwrap();
        assert_eq!(
            scope(&game).amount(&amount),
            Err(ResolveError::Unsupported(lacks))
        );
    }

    #[rstest]
    #[case::not_kicked(r#""Kicked""#, Ok(false))]
    #[case::your_turn(r#""YourTurn""#, Ok(true))]
    #[case::not(r#"{"Not":"YourTurn"}"#, Ok(false))]
    #[case::all(
        r#"{"All":["YourTurn",{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}]}"#,
        Ok(true)
    )]
    #[case::all_with_one_false(
        r#"{"All":["YourTurn",{"Compare":{"left":{"Life":"You"},"op":"Gt","right":20}}]}"#,
        Ok(false)
    )]
    #[case::any(
        r#"{"Any":[{"Not":"YourTurn"},{"Compare":{"left":1,"op":"Lt","right":2}}]}"#,
        Ok(true)
    )]
    #[case::matches(
        r#"{"Matches":{"subject":{"Each":[{"Named":"Ogre"}]},"selector":[{"Type":"Creature"}]}}"#,
        Ok(true)
    )]
    #[case::does_not_match(
        r#"{"Matches":{"subject":{"Each":[{"Named":"Ogre"}]},"selector":[{"Color":"Green"}]}}"#,
        Ok(false)
    )]
    fn a_condition_is_true_or_false_or_unsupported(
        #[case] json: &str,
        #[case] expected: Result<bool, &'static str>,
    ) {
        let game = game();
        let condition: Condition = serde_json::from_str(json).unwrap();
        assert_eq!(
            scope(&game).holds(&condition),
            expected.map_err(ResolveError::Unsupported)
        );
    }

    #[test]
    fn an_object_that_changed_zones_is_a_new_object_and_the_old_one_is_read_from_its_snapshot() {
        let mut game = game();
        let old = named(&game, "Ogre", 1);
        let mut shot = snapshot(&game, old.id).unwrap();
        // The ogre was bigger when it died than its card says.
        shot.characteristics.power = Some(7);
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: old.id,
                from: Zone::Battlefield,
                to: Zone::Graveyard(PlayerId(1)),
                cause: ZoneCause::Destroyed,
            },
        )
        .unwrap();
        assert_eq!(game.object(old.id).unwrap().counter(), 1);
        let bindings: Bindings = [("ogre".to_owned(), vec![Target::Object(old)])].into();
        let power = |scope: &Scope<'_>| -> i64 {
            let amount: Amount =
                serde_json::from_str(r#"{"Stat":{"of":{"Bound":"ogre"},"stat":"Power"}}"#).unwrap();
            scope.amount(&amount).unwrap()
        };
        let creatures = selector(r#"[{"Type":"Creature"}]"#);
        let mut scope = scope(&game);
        scope.bindings = &bindings;
        assert_eq!(
            power(&scope),
            0,
            "without a snapshot there is nothing to read"
        );
        assert!(!scope.selects(&creatures, old).unwrap());
        scope.snapshots = vec![&shot];
        assert_eq!(power(&scope), 7, "the snapshot says what it was");
        assert!(
            scope.selects(&creatures, old).unwrap(),
            "it was a creature when it left"
        );
        // Its controller as it was, too.
        let seats: Ref = serde_json::from_str(r#"{"ControllerOf":{"Bound":"ogre"}}"#).unwrap();
        assert_eq!(scope.players(&seats).unwrap(), [PlayerId(1)]);
    }

    #[test]
    fn a_target_an_earlier_step_moved_stays_named_and_is_read_as_it_was() {
        let mut game = game();
        let (ogre, bear) = (named(&game, "Ogre", 1), named(&game, "Bear", 1));
        let mut shot = snapshot(&game, ogre.id).unwrap();
        shot.characteristics.power = Some(7);
        let choices = Choices {
            targets: vec![vec![Target::Object(ogre), Target::Object(bear)]],
            ..Choices::default()
        };
        let specs = [TargetSpec {
            what: Targetable::Object(selector(r#"[{"Type":"Creature"}]"#)),
            bounds: Bounds::default(),
            divide: None,
        }];
        for object in [ogre, bear] {
            apply_event(
                &mut game,
                &Event::ZoneChanged {
                    object: object.id,
                    from: Zone::Battlefield,
                    to: Zone::Exile,
                    cause: ZoneCause::Exiled,
                },
            )
            .unwrap();
        }
        let mut scope = scope(&game);
        scope.choices = &choices;
        scope.specs = specs.iter().collect();
        let first = Ref::Target(0);
        assert_eq!(
            scope.refs(&first).unwrap(),
            [],
            "both left the battlefield, so neither is a legal target now"
        );
        let departed = [shot];
        scope.departed = &departed;
        scope.snapshots = vec![&departed[0]];
        assert_eq!(
            scope.refs(&first).unwrap(),
            [Target::Object(ogre)],
            "the step that moved the Ogre keeps it named; the Bear is only illegal"
        );
        let power: Amount =
            serde_json::from_str(r#"{"Stat":{"of":{"Target":0},"stat":"Power"}}"#).unwrap();
        assert_eq!(scope.amount(&power), Ok(7), "its power as it was");
        let seats: Ref = serde_json::from_str(r#"{"ControllerOf":{"Target":0}}"#).unwrap();
        assert_eq!(scope.players(&seats).unwrap(), [PlayerId(1)]);
        assert_eq!(scope.amount(&Amount::TargetCount(0)), Ok(1));
    }

    #[test]
    fn live_objects_are_the_named_objects_still_there_each_once() {
        let mut game = game();
        let (ogre, bear) = (named(&game, "Ogre", 1), named(&game, "Bear", 1));
        let bindings: Bindings = [(
            "them".to_owned(),
            vec![
                Target::Object(ogre),
                Target::Object(bear),
                Target::Object(ogre),
                Target::Player(PlayerId(1)),
            ],
        )]
        .into();
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: bear.id,
                from: Zone::Battlefield,
                to: Zone::Exile,
                cause: ZoneCause::Exiled,
            },
        )
        .unwrap();
        let mut scope = scope(&game);
        scope.bindings = &bindings;
        let them: Ref = serde_json::from_str(r#"{"Bound":"them"}"#).unwrap();
        let live: Vec<ObjectId> = scope
            .live_objects(&them)
            .unwrap()
            .iter()
            .map(|object| object.id())
            .collect();
        assert_eq!(live, [ogre.id]);
    }

    #[test]
    fn the_cause_of_a_trigger_is_what_triggering_names() {
        let game = game();
        let cause = crate::abilities::Cause {
            subjects: vec![Target::Player(PlayerId(1))],
            snapshots: Vec::new(),
            amount: 0,
        };
        let mut scope = scope(&game);
        let triggering: Ref = serde_json::from_str(r#""Triggering""#).unwrap();
        assert!(matches!(
            scope.refs(&triggering),
            Err(ResolveError::Invalid(_))
        ));
        scope.cause = Some(&cause);
        assert_eq!(scope.players(&triggering).unwrap(), [PlayerId(1)]);
    }

    #[test]
    fn a_name_nothing_bound_is_an_error_not_an_empty_set() {
        let game = game();
        let ghost: Ref = serde_json::from_str(r#"{"Bound":"ghost"}"#).unwrap();
        assert!(matches!(
            scope(&game).refs(&ghost),
            Err(ResolveError::Invalid(_))
        ));
    }

    #[test]
    fn only_creatures_planeswalkers_and_battles_can_be_dealt_damage() {
        let game = game();
        let scope = scope(&game);
        assert!(scope.can_be_dealt_damage(named(&game, "Bear", 0)));
        assert!(!scope.can_be_dealt_damage(named(&game, "Relic", 0)));
        let in_hand = game
            .object(game.zone(Zone::Hand(PlayerId(0)))[0])
            .unwrap()
            .reference();
        assert!(!scope.can_be_dealt_damage(in_hand));
        let stale = ObjectRef {
            counter: 9,
            ..named(&game, "Bear", 0)
        };
        assert!(!scope.can_be_dealt_damage(stale));
    }

    /// Seat 0's Bear has two +1/+1 counters and a charge counter, seat 1's Ogre a -1/-1 counter; seat 0 has three poison
    /// counters and seat 1 four energy counters.
    fn counted() -> GameState {
        let mut game = game();
        let (bear, ogre) = (named(&game, "Bear", 0), named(&game, "Ogre", 1));
        for (on, counter, delta) in [
            (Target::Object(bear), CounterKind::PlusOnePlusOne, 2),
            (Target::Object(bear), CounterKind::Charge, 1),
            (Target::Object(ogre), CounterKind::MinusOneMinusOne, 1),
            (Target::Player(PlayerId(0)), CounterKind::Poison, 3),
            (
                Target::Player(PlayerId(1)),
                CounterKind::Other("energy".into()),
                4,
            ),
        ] {
            let event = Event::CountersChanged(CountersChanged { on, counter, delta });
            apply_event(&mut game, &event).unwrap();
        }
        game
    }

    const MY_BEAR: &str = r#"{"Each":[{"Named":"Bear"},{"ControlledBy":"You"}]}"#;

    #[rstest]
    #[case::the_object_has_two(MY_BEAR, r#""+1/+1""#, 2)]
    #[case::of_another_kind(MY_BEAR, r#""charge""#, 1)]
    #[case::of_a_kind_it_lacks(MY_BEAR, r#""-1/-1""#, 0)]
    #[case::another_object(r#"{"Each":[{"Named":"Ogre"}]}"#, r#""-1/-1""#, 1)]
    #[case::the_source_holds_none("\"This\"", r#""+1/+1""#, 0)]
    #[case::all_the_objects_named_add_up(r#"{"Each":[{"Type":"Creature"}]}"#, r#""+1/+1""#, 2)]
    #[case::a_player_you(r#""You""#, r#""poison""#, 3)]
    #[case::a_named_kind(r#""Opponents""#, r#"{"Other":"energy"}"#, 4)]
    #[case::each_player_adds_up(r#""EachPlayer""#, r#""poison""#, 3)]
    #[case::a_player_without_any(r#""Opponents""#, r#""poison""#, 0)]
    #[case::nothing_named(r#"{"Each":[{"Named":"Nobody"}]}"#, r#""poison""#, 0)]
    fn counters_are_the_number_the_object_or_player_holds(
        #[case] on: &str,
        #[case] counter: &str,
        #[case] expected: i64,
    ) {
        let game = counted();
        let json = format!(r#"{{"Counters":{{"on":{on},"counter":{counter}}}}}"#);
        let amount: Amount = serde_json::from_str(&json).unwrap();
        assert_eq!(scope(&game).amount(&amount), Ok(expected));
        let condition = format!(r#"{{"Compare":{{"left":{json},"op":"Ge","right":1}}}}"#);
        let condition: Condition = serde_json::from_str(&condition).unwrap();
        assert_eq!(scope(&game).holds(&condition), Ok(expected >= 1));
    }

    #[rstest]
    #[case::a_plus_counter(r#"[{"HasCounter":"+1/+1"}]"#, &["Bear 0"])]
    #[case::a_charge_counter(r#"[{"HasCounter":"charge"}]"#, &["Bear 0"])]
    #[case::a_minus_counter(r#"[{"HasCounter":"-1/-1"}]"#, &["Ogre 1"])]
    #[case::a_kind_nothing_has(r#"[{"HasCounter":"loyalty"}]"#, &[])]
    #[case::no_plus_counter(r#"[{"Type":"Creature"},{"Not":{"HasCounter":"+1/+1"}}]"#, &["Ogre 1", "Bear 1"])]
    #[case::either_kind(r#"[{"AnyOf":[{"HasCounter":"+1/+1"},{"HasCounter":"-1/-1"}]}]"#, &["Bear 0", "Ogre 1"])]
    #[case::with_the_rest_of_the_selector(r#"[{"ControlledBy":"Opponents"},{"HasCounter":"+1/+1"}]"#, &[])]
    fn a_selector_over_counters_matches_the_permanents_that_hold_some(
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(matched(&counted(), json), expected);
    }

    #[test]
    fn the_counters_of_an_object_that_left_are_read_from_its_snapshot() {
        let mut game = counted();
        let old = named(&game, "Bear", 0);
        let shot = snapshot(&game, old.id).unwrap();
        assert_eq!(
            shot.counters,
            [(CounterKind::PlusOnePlusOne, 2), (CounterKind::Charge, 1)]
        );
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: old.id,
                from: Zone::Battlefield,
                to: Zone::Graveyard(PlayerId(0)),
                cause: ZoneCause::Destroyed,
            },
        )
        .unwrap();
        let bindings: Bindings = [("bear".to_owned(), vec![Target::Object(old)])].into();
        let plus: Amount =
            serde_json::from_str(r#"{"Counters":{"on":{"Bound":"bear"},"counter":"+1/+1"}}"#)
                .unwrap();
        let selector = selector(r#"[{"HasCounter":"charge"}]"#);
        let mut scope = scope(&game);
        scope.bindings = &bindings;
        assert_eq!(
            scope.amount(&plus),
            Ok(0),
            "the new object holds none and nothing says otherwise"
        );
        assert!(!scope.selects(&selector, old).unwrap());
        scope.snapshots = vec![&shot];
        assert_eq!(scope.amount(&plus), Ok(2), "the snapshot says what it held");
        assert!(scope.selects(&selector, old).unwrap());
    }

    fn walker() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Walker")
                .with_type(CardType::Planeswalker)
                .with_loyalty(4),
        )
    }

    /// Seat 0 controls a Walker with 3 loyalty counters (printed 4), a Bear and a Relic; a second Walker waits in its hand.
    fn walkers() -> GameState {
        let mut game = state(
            vec![vec![walker()], vec![]],
            vec![vec![walker(), bear(), relic()], vec![ogre()]],
        );
        let on = Target::Object(named(&game, "Walker", 0));
        let event = Event::CountersChanged(CountersChanged {
            on,
            counter: CounterKind::Loyalty,
            delta: -1,
        });
        apply_event(&mut game, &event).unwrap();
        game
    }

    #[rstest]
    #[case::the_counters_on_the_battlefield(r#"{"Each":[{"Named":"Walker"}]}"#, 3)]
    #[case::the_printed_number_in_a_hand(r#"{"Each":[{"Named":"Walker"},{"In":"Hand"}]}"#, 4)]
    #[case::a_creature_has_none(r#"{"Each":[{"Named":"Bear"}]}"#, 0)]
    #[case::nothing_named(r#"{"Each":[{"Named":"Nobody"}]}"#, 0)]
    fn the_loyalty_of_a_planeswalker_is_its_counters_in_play_and_its_printed_number_elsewhere(
        #[case] of: &str,
        #[case] expected: i64,
    ) {
        let game = walkers();
        let json = format!(r#"{{"Stat":{{"of":{of},"stat":"Loyalty"}}}}"#);
        let amount: Amount = serde_json::from_str(&json).unwrap();
        assert_eq!(scope(&game).amount(&amount), Ok(expected));
    }

    #[rstest]
    #[case::at_least_three(r#"[{"Compare":{"stat":"Loyalty","op":"Ge","to":3}}]"#, &["Walker 0"])]
    #[case::at_least_four(r#"[{"Compare":{"stat":"Loyalty","op":"Ge","to":4}}]"#, &[])]
    #[case::at_most_zero(r#"[{"Compare":{"stat":"Loyalty","op":"Le","to":0}}]"#, &[])]
    #[case::only_walkers_have_loyalty(r#"[{"Compare":{"stat":"Loyalty","op":"Ge","to":0}}]"#, &["Walker 0"])]
    fn a_selector_compares_loyalty_and_only_a_planeswalker_has_it(
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(matched(&walkers(), json), expected);
    }

    #[test]
    fn the_loyalty_of_a_planeswalker_that_left_is_read_from_its_snapshot() {
        let mut game = walkers();
        let old = named(&game, "Walker", 0);
        let shot = snapshot(&game, old.id).unwrap();
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: old.id,
                from: Zone::Battlefield,
                to: Zone::Graveyard(PlayerId(0)),
                cause: ZoneCause::StateBased,
            },
        )
        .unwrap();
        let bindings: Bindings = [("walker".to_owned(), vec![Target::Object(old)])].into();
        let loyalty: Amount =
            serde_json::from_str(r#"{"Stat":{"of":{"Bound":"walker"},"stat":"Loyalty"}}"#).unwrap();
        let mut scope = scope(&game);
        scope.bindings = &bindings;
        assert_eq!(
            scope.amount(&loyalty),
            Ok(0),
            "the card in the graveyard is another object and nothing says otherwise"
        );
        scope.snapshots = vec![&shot];
        assert_eq!(
            scope.amount(&loyalty),
            Ok(3),
            "the snapshot says what it held"
        );
    }

    #[rstest]
    #[case::devotion(r#"{"Devotion":["Black"]}"#, "devotion")]
    fn an_amount_the_evaluator_cannot_read_yet_says_what_it_lacks(
        #[case] json: &str,
        #[case] lacks: &'static str,
    ) {
        let game = game();
        let amount: Amount = serde_json::from_str(json).unwrap();
        assert_eq!(
            scope(&game).amount(&amount),
            Err(ResolveError::Unsupported(lacks))
        );
    }

    #[rstest]
    #[case::you_when_nobody_is(r#"{"Monarch":"You"}"#, None, false)]
    #[case::you_when_you_are(r#"{"Monarch":"You"}"#, Some(0), true)]
    #[case::you_when_an_opponent_is(r#"{"Monarch":"You"}"#, Some(1), false)]
    #[case::an_opponent_when_they_are(r#"{"Monarch":"Opponents"}"#, Some(1), true)]
    #[case::an_opponent_when_you_are(r#"{"Monarch":"Opponents"}"#, Some(0), false)]
    #[case::not_the_monarch(r#"{"Not":{"Monarch":"You"}}"#, Some(1), true)]
    fn the_monarch_condition_reads_the_designation(
        #[case] json: &str,
        #[case] monarch: Option<usize>,
        #[case] holds: bool,
    ) {
        let mut game = game();
        if let Some(seat) = monarch {
            let player = PlayerId::from_index(seat);
            crate::fold::apply_event(&mut game, &Event::MonarchChanged { player }).unwrap();
        }
        let condition: Condition = serde_json::from_str(json).unwrap();
        assert_eq!(scope(&game).holds(&condition), Ok(holds));
    }

    #[rstest]
    #[case::new_to_its_controller(r#"{"NewlyControlled":"This"}"#, true, 0, true)]
    #[case::held_since_the_turn_began(r#"{"NewlyControlled":"This"}"#, false, 0, false)]
    #[case::the_object_is_not_the_controllers(r#"{"NewlyControlled":"This"}"#, true, 1, false)]
    #[case::not(r#"{"Not":{"NewlyControlled":"This"}}"#, true, 0, false)]
    #[case::nothing_named(
        r#"{"NewlyControlled":{"Each":[{"Named":"Nothing"}]}}"#,
        true,
        0,
        false
    )]
    fn a_permanent_new_to_its_controller_is_read_from_what_the_turn_recorded(
        #[case] json: &str,
        #[case] fresh: bool,
        #[case] controller: usize,
        #[case] holds: bool,
    ) {
        let mut game = game();
        let relic = named(&game, "Relic", 0);
        let permanent = game
            .objects
            .get_mut(&relic.id)
            .and_then(|object| object.permanent.as_mut())
            .unwrap();
        permanent.new_to = fresh.then_some(PlayerId(0));
        let condition: Condition = serde_json::from_str(json).unwrap();
        let scope = Scope::announcing(&game, PlayerId::from_index(controller), relic);
        assert_eq!(scope.holds(&condition), Ok(holds));
    }

    #[test]
    fn a_permanent_that_changed_zones_is_not_the_one_a_condition_named() {
        let mut game = game();
        let relic = named(&game, "Relic", 0);
        let permanent = game
            .objects
            .get_mut(&relic.id)
            .and_then(|object| object.permanent.as_mut())
            .unwrap();
        permanent.new_to = Some(PlayerId(0));
        let condition: Condition = serde_json::from_str(r#"{"NewlyControlled":"This"}"#).unwrap();
        crate::fold::apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: relic.id,
                from: Zone::Battlefield,
                to: Zone::Hand(PlayerId(0)),
                cause: crate::events::ZoneCause::Moved,
            },
        )
        .unwrap();
        let scope = Scope::announcing(&game, PlayerId(0), relic);
        assert_eq!(scope.holds(&condition), Ok(false));
    }

    #[rstest]
    #[case::if_you_do(r#""Performed""#, "\"if you do\"")]
    fn a_condition_the_evaluator_cannot_read_yet_says_what_it_lacks(
        #[case] json: &str,
        #[case] lacks: &'static str,
    ) {
        let game = game();
        let condition: Condition = serde_json::from_str(json).unwrap();
        assert_eq!(
            scope(&game).holds(&condition),
            Err(ResolveError::Unsupported(lacks))
        );
    }
}
