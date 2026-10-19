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
            // An object that has changed zones since it was named keeps the keywords its card prints.
            Pred::HasKeyword(keyword) => seen.card.is_some_and(|card| {
                if card.counter() == reference.counter {
                    keywords::has_keyword(state, reference.id, keyword)
                } else {
                    keywords::printed(card).any(|held| held == keyword)
                }
            }),
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

#[cfg(test)]
mod tests {
    //! Goal: references, selectors, amounts and conditions read the game as the rules say, an object that changed zones is read as it was when a snapshot of it exists, and what the engine cannot evaluate yet refuses by name.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::{CardType, Supertype};
    use mtg_core::ids::ObjectId;
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;
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
    #[case::nothing_is_attached_yet(r#""Attached""#, &[])]
    fn a_reference_names_its_players_and_objects(#[case] json: &str, #[case] expected: &[&str]) {
        let game = game();
        let reference: Ref = serde_json::from_str(json).unwrap();
        let found = scope(&game).refs(&reference).unwrap();
        assert_eq!(names(&game, &found), expected);
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
    #[case::sum(r#"{"Sum":[2,{"Count":[{"Type":"Artifact"}]},-1]}"#, 2)]
    #[case::no_targets_chosen(r#"{"TargetCount":0}"#, 0)]
    fn an_amount_is_the_number_it_stands_for(#[case] json: &str, #[case] expected: i64) {
        let game = game();
        let amount: Amount = serde_json::from_str(json).unwrap();
        assert_eq!(scope(&game).amount(&amount), Ok(expected));
    }

    #[rstest]
    #[case::x(r#""X""#, "X")]
    #[case::counters(r#"{"Counters":{"on":"This","counter":"+1/+1"}}"#, "counters")]
    #[case::divided(r#""Divided""#, "divided amounts")]
    #[case::even_share(r#"{"EvenShare":4}"#, "divided amounts")]
    #[case::loyalty(
        r#"{"Stat":{"of":{"Each":[{"Named":"Ogre"}]},"stat":"Loyalty"}}"#,
        "loyalty counters"
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
    #[case::kicked(r#""Kicked""#, Err("kicker"))]
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
    fn the_cause_of_a_trigger_is_what_triggering_names() {
        let game = game();
        let cause = crate::abilities::Cause {
            subjects: vec![Target::Player(PlayerId(1))],
            snapshots: Vec::new(),
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
}
