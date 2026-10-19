//! The values of the DSL read from the game: references, selectors, amounts and conditions.
//!
//! Nothing here changes state. What the engine cannot evaluate yet (counters, X, divided amounts, kicker)
//! is a `ResolveError::Unsupported` with the name of what is missing; `Caps` keeps the cards that need
//! them from being offered, so the error is not met in a game.

use std::collections::BTreeMap;

use mtg_cards::refs::{Pred, Ref, Selector, TargetSpec, ZoneKind};
use mtg_cards::value::{Amount, Condition, StatKind};
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use thiserror::Error;

use crate::abilities::Cause;
use crate::characteristics::{Characteristics, characteristics};
use crate::choose::{Choices, Target, is_legal};
use crate::keywords;
use crate::object::{GameObject, ObjectKind, ObjectRef};
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
};
static NO_BINDINGS: Bindings = BTreeMap::new();

/// Everything a value may depend on: the game, who is acting for which object, and what the object announced and bound.
#[derive(Debug, Clone)]
pub struct Scope<'a> {
    pub(crate) state: &'a GameState,
    pub(crate) controller: PlayerId,
    pub(crate) source: ObjectRef,
    pub(crate) choices: &'a Choices,
    pub(crate) specs: &'a [TargetSpec],
    pub(crate) bindings: &'a Bindings,
    pub(crate) snapshots: Vec<&'a Snapshot>,
    pub(crate) cause: Option<&'a Cause>,
}

/// What is known of an object: the object itself if it is still the one that was named, else how it was.
struct Seen<'a> {
    owner: PlayerId,
    controller: PlayerId,
    zone: Zone,
    kind: ObjectKind,
    chars: Characteristics,
    tapped: Option<bool>,
    /// The same card, whichever zone it has moved to since: its printed side does not change.
    card: Option<&'a GameObject>,
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
            specs: &[],
            bindings: &NO_BINDINGS,
            snapshots: Vec::new(),
            cause: None,
        }
    }

    fn seen(&self, reference: ObjectRef) -> Option<Seen<'a>> {
        let live = self.state.object(reference.id);
        if let Some(object) = live.filter(|o| o.counter() == reference.counter) {
            return Some(Seen {
                owner: object.owner(),
                controller: object.controller(),
                zone: object.zone(),
                kind: object.kind(),
                chars: characteristics(self.state, reference.id)?,
                tapped: object
                    .permanent()
                    .map(crate::object::PermanentState::tapped),
                card: Some(object),
            });
        }
        let shot = self
            .snapshots
            .iter()
            .find(|s| s.id == reference.id && s.counter == reference.counter)?;
        Some(Seen {
            owner: shot.owner,
            controller: shot.controller,
            zone: shot.zone,
            kind: live.map_or(ObjectKind::Card, GameObject::kind),
            chars: shot.characteristics.clone(),
            tapped: None,
            card: live,
        })
    }

    /// The legal targets chosen for the specification `index`: an illegal one is ignored (CR 608.2b).
    fn chosen(&self, index: usize) -> Vec<Target> {
        let (Some(set), Some(spec)) = (self.choices.targets.get(index), self.specs.get(index))
        else {
            return Vec::new();
        };
        set.iter()
            .copied()
            .filter(|target| is_legal(self.state, spec, *target, self.source.id, self.controller))
            .collect()
    }

    /// CR 608.2b: whether targets were chosen and every one of them is illegal now, so the spell or ability does nothing.
    pub(crate) fn all_targets_illegal(&self) -> bool {
        let mut any = false;
        for (index, set) in self.choices.targets.iter().enumerate() {
            let Some(spec) = self.specs.get(index) else {
                continue;
            };
            for target in set {
                any = true;
                if is_legal(self.state, spec, *target, self.source.id, self.controller) {
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
            Ref::Target(index) => self.chosen(usize::from(*index)),
            Ref::Triggering => self
                .cause
                .map(|cause| cause.subjects.clone())
                .ok_or_else(|| ResolveError::Invalid("there is no triggering event".into()))?,
            Ref::Attached => Vec::new(),
            Ref::Bound(name) => self
                .bindings
                .get(name)
                .cloned()
                .ok_or_else(|| ResolveError::Invalid(format!("`{name}` is not bound")))?,
            Ref::ControllerOf(inner) => self.seats_of(inner, |seen| seen.controller)?,
            Ref::OwnerOf(inner) => self.seats_of(inner, |seen| seen.owner)?,
            Ref::Each(selector) => self
                .matching(selector)?
                .into_iter()
                .map(Target::Object)
                .collect(),
        })
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
            None if selector.0.contains(&Pred::Spell) => vec![Zone::Stack],
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
            Pred::Type(card_type) => seen.chars.types.contains(*card_type),
            Pred::Supertype(supertype) => seen
                .card
                .is_some_and(|c| c.card().supertypes.contains(supertype)),
            Pred::Subtype(subtype) => seen
                .card
                .is_some_and(|c| c.card().subtypes.contains(subtype)),
            Pred::Color(color) => seen.chars.colors.contains(*color),
            Pred::Colorless => seen.chars.colors.is_empty(),
            Pred::Token => seen.kind == ObjectKind::Token,
            Pred::Spell => seen.zone == Zone::Stack && seen.kind != ObjectKind::Ability,
            Pred::Tapped => seen.tapped == Some(true),
            Pred::Untapped => seen.tapped == Some(false),
            Pred::Attacking => state
                .combat()
                .attackers()
                .iter()
                .any(|a| a.object() == reference.id),
            Pred::Blocking => state
                .combat()
                .attackers()
                .iter()
                .any(|a| a.blockers().contains(&reference.id)),
            Pred::ControlledBy(who) => self.players(who)?.contains(&seen.controller),
            Pred::OwnedBy(who) => self.players(who)?.contains(&seen.owner),
            Pred::In(kind) => zone_kind(seen.zone) == *kind,
            Pred::HasKeyword(keyword) => seen
                .card
                .is_some_and(|c| keywords::held_by(c).any(|held| held == keyword)),
            Pred::Named(name) => seen.card.is_some_and(|c| c.card().name == *name),
            Pred::Compare { stat, op, to } => match Self::stat(seen, *stat)? {
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

    fn stat(seen: &Seen<'_>, stat: StatKind) -> Result<Option<i64>, ResolveError> {
        Ok(match stat {
            StatKind::Power => seen.chars.power.map(i64::from),
            StatKind::Toughness => seen.chars.toughness.map(i64::from),
            StatKind::ManaValue => seen.card.map(|c| i64::from(c.card().mana_value())),
            StatKind::Loyalty => return Err(ResolveError::Unsupported("loyalty counters")),
        })
    }

    /// The number an amount stands for.
    pub fn amount(&self, amount: &Amount) -> Result<i64, ResolveError> {
        Ok(match amount {
            Amount::Fixed(n) => i64::from(*n),
            Amount::X => self
                .choices
                .x
                .map(i64::from)
                .ok_or(ResolveError::Unsupported("X"))?,
            Amount::Count(selector) => i64::try_from(self.matching(selector)?.len())
                .map_err(|_| ResolveError::Invalid("too many objects".into()))?,
            Amount::Stat { of, stat } => match self.objects(of)?.first() {
                Some(object) => self
                    .seen(*object)
                    .map(|seen| Self::stat(&seen, *stat))
                    .transpose()?
                    .flatten()
                    .unwrap_or(0),
                None => 0,
            },
            Amount::Life(who) => self
                .players(who)?
                .first()
                .and_then(|p| self.state.player(*p))
                .map_or(0, |p| i64::from(p.life())),
            Amount::Counters { .. } => return Err(ResolveError::Unsupported("counters")),
            Amount::Sum(parts) => {
                let mut sum = 0i64;
                for part in parts {
                    sum += self.amount(part)?;
                }
                sum
            }
            Amount::Divided | Amount::EvenShare(_) => {
                return Err(ResolveError::Unsupported("divided amounts"));
            }
            Amount::TargetCount(index) => i64::try_from(self.chosen(usize::from(*index)).len())
                .map_err(|_| ResolveError::Invalid("too many targets".into()))?,
        })
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
            Condition::Kicked => return Err(ResolveError::Unsupported("kicker")),
            Condition::YourTurn => self.state.turn().active() == self.controller,
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
