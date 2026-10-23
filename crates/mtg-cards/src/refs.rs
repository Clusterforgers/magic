//! References to players and objects, selectors over the game, and target specifications.

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
use serde::{Deserialize, Serialize};

use crate::keyword::Keyword;
use crate::value::{Amount, Cmp, CounterKind, StatKind};

/// A zone a selector or a pattern can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ZoneKind {
    /// The battlefield (the default place to look).
    #[default]
    Battlefield,
    /// A hand.
    Hand,
    /// A library.
    Library,
    /// A graveyard.
    Graveyard,
    /// The stack.
    Stack,
    /// Exile.
    Exile,
    /// The command zone.
    Command,
}

impl ZoneKind {
    /// Whether this is the default zone, so serialization can leave it out.
    pub const fn is_battlefield(&self) -> bool {
        matches!(self, ZoneKind::Battlefield)
    }

    /// The default zone a search looks in.
    pub const fn library() -> ZoneKind {
        ZoneKind::Library
    }

    /// Whether this is a library, the default zone of a search.
    pub const fn is_library(&self) -> bool {
        matches!(self, ZoneKind::Library)
    }
}

/// A player or object an effect, pattern or value talks about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Ref {
    /// The controller of the spell or ability.
    You,
    /// Every opponent of the controller.
    Opponents,
    /// Every player.
    EachPlayer,
    /// The source object of the ability (the spell itself for a spell).
    This,
    /// The targets chosen for the target specification with this index.
    Target(u8),
    /// The player or object the trigger event is about ("that creature", "that player").
    Triggering,
    /// What the source is attached to ("enchanted creature", "equipped creature").
    Attached,
    /// The player the source is attached to ("enchanted player").
    AttachedPlayer,
    /// The monarch ("the monarch"); nobody while no player is the monarch.
    Monarch,
    /// The opponent the gift was promised to as the spell the ability reads was cast (CR 702.174); nobody when none was promised or they have left the game.
    Promised,
    /// The defending player of the attacking creature that triggered the ability, else of the source: whoever it
    /// attacks, or controls the planeswalker it attacks (CR 506.3); still named once the creature has left combat.
    Defending,
    /// The opponents of the controller that their creatures attacked this combat, directly or through a planeswalker
    /// ("each opponent you attacked this combat"); a creature that left combat still counts.
    Attacked,
    /// The player with the most life, or each of the players tied for it.
    MostLife,
    /// The result an earlier step bound with `as`.
    Bound(String),
    /// The objects picked for the cost of the ability ("the creature tapped to pay it"): their characteristics are those they
    /// have as the ability resolves, and none is named when the cost picks nothing.
    Paid,
    /// The controller of the referenced object.
    ControllerOf(Box<Ref>),
    /// The owner of the referenced object.
    OwnerOf(Box<Ref>),
    /// Every object matching the selector when the effect runs.
    Each(Selector),
}

impl Ref {
    /// The default `who`: the controller.
    pub const fn you() -> Ref {
        Ref::You
    }

    /// Whether this is the controller, so serialization can leave it out.
    pub const fn is_you(&self) -> bool {
        matches!(self, Ref::You)
    }

    /// The default player scope of a pattern: every player.
    pub const fn each_player() -> Ref {
        Ref::EachPlayer
    }

    /// Whether this is every player, so serialization can leave it out.
    pub const fn is_each_player(&self) -> bool {
        matches!(self, Ref::EachPlayer)
    }
}

/// Which kind of ability on the stack a predicate selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AbilityKind {
    /// An activated ability, a loyalty ability included.
    Activated,
    /// A triggered ability.
    Triggered,
}

/// A condition on one object; selectors hold a list of them that must all be true.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Pred {
    /// Is the source object of the ability.
    This,
    /// Is not the source object.
    Other,
    /// Is one of the objects the reference names (`{"Is": {"Bound": "creature"}}`).
    Is(Ref),
    /// Shares a subtype with one of the objects the reference names (`{"SharesSubtype": {"Bound": "land"}}`).
    SharesSubtype(Ref),
    /// Is a card exiled face down by one of the permanents the reference names, which has not left since (hideaway, CR 702.75a).
    ExiledWith(Ref),
    /// Is a creature tapped this turn to crew one of the Vehicles the reference names ("creature that crewed it this turn").
    Crewed(Ref),
    /// Is a creature tapped this turn to saddle one of the Mounts the reference names ("creature that saddled it this turn").
    Saddled(Ref),
    /// Has the card type.
    Type(CardType),
    /// Has the supertype.
    Supertype(Supertype),
    /// Has the subtype.
    Subtype(Subtype),
    /// Is the color.
    Color(Color),
    /// Is colorless.
    Colorless,
    /// Is a token.
    Token,
    /// Is a player's commander (CR 903.3): a card designated as one when the game began, whoever controls it now.
    Commander,
    /// Is a spell on the stack.
    Spell,
    /// Is an activated or a triggered ability on the stack (a mana ability never is one).
    Ability(AbilityKind),
    /// Is tapped.
    Tapped,
    /// Is untapped.
    Untapped,
    /// Is monstrous (CR 701.37): a permanent on the battlefield that has become monstrous since it entered.
    Monstrous,
    /// Is attacking.
    Attacking,
    /// Is attacking the player, not a planeswalker of theirs ("attacking that player").
    AttackingPlayer(Ref),
    /// Is blocking.
    Blocking,
    /// Is an attacking creature that no creature was declared to block, once its defending player has declared blockers (CR 509.1h).
    Unblocked,
    /// Is controlled by the player.
    ControlledBy(Ref),
    /// Is owned by the player.
    OwnedBy(Ref),
    /// Is in the zone (without it, the battlefield).
    In(ZoneKind),
    /// Is a spell cast from the zone, a hand or a graveyard (the spell being cast included).
    CastFrom(ZoneKind),
    /// Has the keyword.
    HasKeyword(Keyword),
    /// Has at least one counter of the kind on it.
    HasCounter(CounterKind),
    /// Has exactly this card name.
    Named(String),
    /// Is the colour the controller of the source chose as the source entered the battlefield.
    ChosenColor,
    /// Has the creature type the controller of the source chose as the source entered the battlefield.
    ChosenSubtype,
    /// Its power, toughness, mana value or loyalty compares to the amount.
    Compare {
        /// The characteristic.
        stat: StatKind,
        /// The comparison.
        op: Cmp,
        /// The amount compared with.
        to: Amount,
    },
    /// The predicate is false.
    Not(Box<Pred>),
    /// At least one of the predicates is true.
    AnyOf(Vec<Pred>),
}

/// A description of a set of objects: all predicates must hold. The empty list matches everything.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Selector(pub Vec<Pred>);

impl Selector {
    /// The selector that matches only the source object.
    pub fn this() -> Selector {
        Selector(vec![Pred::This])
    }

    /// Whether the selector has no predicates.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// What a target can be.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Targetable {
    /// Any target: a creature, planeswalker, battle or player.
    AnyTarget,
    /// A player.
    Player,
    /// An opponent of the controller.
    Opponent,
    /// An object matching the selector, in the zone the selector names.
    Object(Selector),
    /// Any of the listed kinds.
    Either(Vec<Targetable>),
}

/// How many of something: at least `min`, at most `max`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    /// The fewest allowed.
    #[serde(default = "Bounds::one_min")]
    pub min: u32,
    /// The most allowed; `null` means any number.
    #[serde(default = "Bounds::one_max")]
    pub max: Option<Amount>,
}

impl Bounds {
    const fn one_min() -> u32 {
        1
    }

    // serde's `default` must return the field's own type, which is an `Option` here.
    #[allow(clippy::unnecessary_wraps)]
    fn one_max() -> Option<Amount> {
        Some(Amount::Fixed(1))
    }

    /// Whether this is exactly one, so serialization can leave it out.
    pub fn is_one(&self) -> bool {
        *self == Bounds::default()
    }
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            min: 1,
            max: Some(Amount::Fixed(1)),
        }
    }
}

/// One "target ..." of a spell or ability.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSpec {
    /// What may be targeted.
    pub what: Targetable,
    /// How many targets are chosen.
    #[serde(default, skip_serializing_if = "Bounds::is_one")]
    pub bounds: Bounds,
    /// The amount divided among the targets as the controller chooses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divide: Option<Amount>,
}

#[cfg(test)]
mod tests {
    //! Goal: references, selectors and target specifications use the compact JSON forms the card files rely on.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::unit(Ref::You, r#""You""#)]
    #[case::target(Ref::Target(1), r#"{"Target":1}"#)]
    #[case::bound(Ref::Bound("found".into()), r#"{"Bound":"found"}"#)]
    #[case::nested(
        Ref::ControllerOf(Box::new(Ref::Target(0))),
        r#"{"ControllerOf":{"Target":0}}"#
    )]
    #[case::each(
        Ref::Each(Selector(vec![Pred::Type(CardType::Creature), Pred::Other])),
        r#"{"Each":[{"Type":"Creature"},"Other"]}"#
    )]
    fn refs_have_stable_json(#[case] value: Ref, #[case] json: &str) {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(serde_json::from_str::<Ref>(json).unwrap(), value);
    }

    #[rstest]
    #[case::a_plus_counter(
        Pred::HasCounter(CounterKind::PlusOnePlusOne),
        r#"{"HasCounter":"+1/+1"}"#
    )]
    #[case::a_named_counter(
        Pred::HasCounter(CounterKind::Other("quest".into())),
        r#"{"HasCounter":{"Other":"quest"}}"#
    )]
    #[case::no_counter(
        Pred::Not(Box::new(Pred::HasCounter(CounterKind::Poison))),
        r#"{"Not":{"HasCounter":"poison"}}"#
    )]
    fn predicates_over_counters_have_stable_json(#[case] value: Pred, #[case] json: &str) {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(serde_json::from_str::<Pred>(json).unwrap(), value);
    }

    #[test]
    fn a_target_spec_leaves_out_its_defaults() {
        let spec = TargetSpec {
            what: Targetable::Opponent,
            bounds: Bounds::default(),
            divide: None,
        };
        assert_eq!(
            serde_json::to_string(&spec).unwrap(),
            r#"{"what":"Opponent"}"#
        );
        let any = TargetSpec {
            what: Targetable::AnyTarget,
            bounds: Bounds { min: 0, max: None },
            divide: Some(Amount::Fixed(2)),
        };
        let json = serde_json::to_string(&any).unwrap();
        assert_eq!(
            json,
            r#"{"what":"AnyTarget","bounds":{"min":0,"max":null},"divide":2}"#
        );
        assert_eq!(serde_json::from_str::<TargetSpec>(&json).unwrap(), any);
    }

    #[test]
    fn a_missing_bound_is_one_and_null_is_unbounded() {
        let one: Bounds = serde_json::from_str("{}").unwrap();
        assert_eq!(one, Bounds::default());
        let any: Bounds = serde_json::from_str(r#"{"min":0,"max":null}"#).unwrap();
        assert_eq!(any.max, None);
    }

    #[test]
    fn unknown_fields_are_refused() {
        let error = serde_json::from_str::<TargetSpec>(r#"{"what":"Player","count":2}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("unknown field `count`"), "{error}");
    }
}
