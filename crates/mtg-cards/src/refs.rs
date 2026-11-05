//! References to players and objects, selectors over the game, and target specifications.

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
use serde::{Deserialize, Serialize};

use crate::keyword::Keyword;
use crate::value::{Amount, CastWay, Cmp, CounterKind, StatKind};

/// The slot of `Ref::Remembered` that holds what the source exiled (`Pred::ExiledWith` reads it).
pub const EXILED_SLOT: &str = "exiled";

/// A zone a selector or a pattern can name.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
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

    /// The default zone a cast comes from.
    pub const fn hand() -> ZoneKind {
        ZoneKind::Hand
    }

    /// Whether this is a hand, the default zone of a cast.
    pub const fn is_hand(&self) -> bool {
        matches!(self, ZoneKind::Hand)
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
    /// The player the trigger event is about, never the object it also holds ("that player" of an activation: damage to it spares the activated permanent).
    TriggeringPlayer,
    /// The object that did what the trigger event is about: the source of the damage ("Whenever a Vampire you control deals
    /// combat damage to a player, put a +1/+1 counter on it"); nobody for an event with no such object.
    Dealer,
    /// What the source is attached to ("enchanted creature", "equipped creature").
    Attached,
    /// The player the source is attached to ("enchanted player").
    AttachedPlayer,
    /// The monarch ("the monarch"); nobody while no player is the monarch.
    Monarch,
    /// The player the source chose as it entered ("the chosen player"); nobody before the choice, for a source that chose no player or once that player has left the game.
    Chosen,
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
    /// The player whose turn it is; nobody once they have left the game.
    ActivePlayer,
    /// The result an earlier step bound with `as`.
    Bound(String),
    /// The objects picked for the cost of the ability ("the creature tapped to pay it"): their characteristics are those they
    /// have as the ability resolves, and none is named when the cost picks nothing.
    Paid,
    /// The object a static ability's change applies to, worked out for each such object ("for each other creature that shares a type with it").
    Affected,
    /// The objects the source named by `on` remembers in `slot` (`Effect::Remember`; the slot `exiled` holds what the source
    /// exiled), as the objects they are now: one that has changed zones since is another object and drops out.
    Remembered {
        /// The object whose memory is read.
        on: Box<Ref>,
        /// The name of the memory.
        slot: String,
    },
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
    /// Has the same name as one of the objects the reference names ("with the same name as that creature"); a nameless
    /// object (a face-down one) shares no name.
    SharesName(Ref),
    /// Shares a card type with one of the objects the reference names ("that shares a card type with it").
    SharesType(Ref),
    /// Shares a color with one of the objects the reference names ("that shares a color with it").
    SharesColor(Ref),
    /// Has {X} in its mana cost ("a spell with {X} in its mana cost", CR 107.3).
    XInCost,
    /// Has a mana cost of exactly that many generic mana and nothing else ("with mana cost {1}"): `{0}` is a cost, no cost
    /// is not, and a coloured, hybrid, Phyrexian or {X} symbol is another cost with the same mana value.
    GenericCost(u32),
    /// Is a creature on the battlefield that some player goads (CR 701.15).
    Goaded,
    /// Has a permanent attached to it that matches the selector ("enchanted": `[{"Subtype":"Aura"}]`, "enchanted by an
    /// Aura you control", "equipped").
    HasAttached(Selector),
    /// Is a permanent attached to one of the objects the reference names ("Aura attached to it", "Equipment attached to
    /// equipped creature").
    AttachedTo(Ref),
    /// Is a card exiled by one of the permanents the reference names, which has not left since: face down by hideaway
    /// (CR 702.75a), or by any effect of that permanent (CR 607.2a), read from its `exiled` memory.
    ExiledWith(Ref),
    /// Is a creature on the battlefield that one of the cards the reference names is encoded on, which has not left since (cipher, CR 702.99).
    EncodedWith(Ref),
    /// Is a creature tapped this turn to crew one of the Vehicles the reference names ("creature that crewed it this turn").
    Crewed(Ref),
    /// Is a creature tapped this turn to saddle one of the Mounts the reference names ("creature that saddled it this turn").
    Saddled(Ref),
    /// Entered the battlefield this turn, from anywhere and under any control (a token made this turn included).
    EnteredThisTurn,
    /// Is a permanent that summoning sickness holds back from attacking and from {T} abilities (CR 302.6).
    SummoningSick,
    /// Is a card in a graveyard that a mill put there this turn ("a land card in a graveyard that was milled this turn").
    MilledThisTurn,
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
    /// Is renowned (CR 702.112): a permanent on the battlefield that has become renowned since it entered.
    Renowned,
    /// Is paired with another creature (CR 702.95): a creature on the battlefield that a soulbond pairing holds.
    Paired,
    /// Is the creature paired with one of the objects the reference names (the partner: "each of those creatures" is `AnyOf` of `This` and `PairedWith(This)`).
    PairedWith(Ref),
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
    /// Has the keyword at any value of its parameter ("with toxic", "with cycling"); the name is Scryfall's spelling
    /// (`Keyword::NAMES`).
    HasKeywordNamed(String),
    /// Has at least one counter of the kind on it.
    HasCounter(CounterKind),
    /// Has at least one counter of any kind on it (CR 122.1).
    HasAnyCounter,
    /// The number of counters of the kind on it compares to `to` ("three or more +1/+1 counters"); none at all counts as zero.
    CounterCount {
        /// The kind of counter counted.
        kind: CounterKind,
        /// The comparison of the number held with `to`.
        op: Cmp,
        /// The number compared with.
        to: u32,
    },
    /// Has exactly this card name.
    Named(String),
    /// Is the colour the controller of the source chose as the source entered the battlefield.
    ChosenColor,
    /// Has the creature type the controller of the source chose as the source entered the battlefield; in a landwalk the land type.
    ChosenSubtype,
    /// Has the card name the controller of the source chose as the source entered the battlefield.
    ChosenName,
    /// Matches the value the resolution remembered under the name (`Effect::ChooseValue`): has the colour, the creature,
    /// land or card type or the card name; a number or a word matches nothing.
    ChosenAs(String),
    /// Its power, toughness, mana value or loyalty compares to the amount.
    Compare {
        /// The characteristic.
        stat: StatKind,
        /// The comparison.
        op: Cmp,
        /// The amount compared with.
        to: Amount,
    },
    /// A spell or ability on the stack with a target that matches the selector. A spell being cast reads the targets
    /// announced so far; the cost judged before any target holds it where some object in play matches.
    Targeting(Selector),
    /// Was dealt damage this turn.
    DamagedThisTurn,
    /// Was declared as an attacker this turn ("creatures that attacked this turn").
    AttackedThisTurn,
    /// Was dealt damage this turn by one of the objects the reference names (the object as it is now: one that left and came back is another).
    DamagedBy(Ref),
    /// Is the card one of the objects the reference names is, in whatever zone it is now: a card that went to a graveyard is a
    /// new object there (CR 400.7), and this finds it again by its card.
    SameCard(Ref),
    /// Is modified (CR 700.9): a permanent with a counter on it, an Equipment attached, or an Aura its controller controls attached.
    Modified,
    /// Lies face down: a face-down permanent, or a card exiled face down.
    FaceDown,
    /// A spell or ability on the stack with a target that is one of the players the reference names ("spell that targets you").
    TargetsPlayer(Ref),
    /// Is a card in a graveyard that was put there from the battlefield this turn ("creature cards in your graveyard that were
    /// put there from the battlefield this turn").
    GravedThisTurn,
    /// Went into the zone it is in now this turn from a zone of the kind ("creature cards put into your graveyard from your
    /// library this turn"); an object that moved again counts only for its latest move, and a token made there came from nowhere.
    ArrivedFrom(ZoneKind),
    /// Is a spell cast, or a permanent that entered as a spell cast, by one of the players `by` names (anyone when absent)
    /// from the kind of zone `from` (anywhere when absent); a copy or a token was never cast.
    WasCast {
        /// The players who cast it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<Ref>,
        /// The zone it was cast from.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<ZoneKind>,
    },
    /// Is a spell, or a permanent that entered as a spell, cast for the alternative cost (kicked, flashed back, evoked).
    CastWith(CastWay),
    /// Is the top card of its owner's library.
    TopOfLibrary,
    /// Is the bottom card of its owner's library.
    BottomOfLibrary,
    /// Is an attacking creature blocked by one of the creatures the reference names.
    BlockedBy(Ref),
    /// Is a creature blocking one of the creatures the reference names.
    Blocks(Ref),
    /// Is a target of one of the spells or abilities on the stack the reference names.
    TargetedBy(Ref),
    /// Is a spell or ability on the stack with exactly one target (CR 115.7: "a spell with a single target").
    SingleTarget,
    /// Has the greatest value of the characteristic among the objects the group selector matches (a tie counts for each).
    GreatestAmong {
        /// The characteristic compared.
        stat: StatKind,
        /// The objects compared; the object itself counts when it matches.
        group: Selector,
    },
    /// Has the least value of the characteristic among the objects the group selector matches (a tie counts for each).
    LeastAmong {
        /// The characteristic compared.
        stat: StatKind,
        /// The objects compared; the object itself counts when it matches.
        group: Selector,
    },
    /// Is a card in exile with a time counter on it and suspend (CR 702.62).
    Suspended,
    /// Is a spell, or a permanent that entered as a spell, cast with at least one kicker cost paid (CR 702.33).
    Kicked,
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
    /// For each player `players` names, an object of `what` that player controls (owns, outside the battlefield): at
    /// most one target of each player ("for each opponent, destroy up to one target artifact that player controls").
    PerPlayer {
        /// The players the targets are chosen for.
        players: Ref,
        /// What each may be, without the player.
        what: Selector,
    },
}

impl Targetable {
    /// The objects a `PerPlayer` lets a spell target, as one selector: `what` controlled by the players, or owned by
    /// them for a selector of another zone.
    pub fn c10_dice_per_player_objects(players: &Ref, what: &Selector) -> Selector {
        let elsewhere = what
            .0
            .iter()
            .any(|pred| matches!(pred, Pred::In(zone) if *zone != ZoneKind::Battlefield));
        let mut all = what.0.clone();
        all.push(if elsewhere {
            Pred::OwnedBy(players.clone())
        } else {
            Pred::ControlledBy(players.clone())
        });
        Selector(all)
    }
}

/// How many of something: at least `min`, at most `max`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    /// The fewest allowed; an amount for "X target creatures".
    #[serde(default = "Amount::one")]
    pub min: Amount,
    /// The most allowed; `null` means any number.
    #[serde(default = "Bounds::one_max")]
    pub max: Option<Amount>,
}

impl Bounds {
    /// Between none and `max`: "up to two", and every number when `max` is `None`.
    pub const fn up_to(max: Option<Amount>) -> Bounds {
        Bounds {
            min: Amount::Fixed(0),
            max,
        }
    }

    /// Exactly `count`, a number or an amount such as X.
    pub fn exactly(count: &Amount) -> Bounds {
        Bounds {
            min: count.clone(),
            max: Some(count.clone()),
        }
    }

    /// The least as a number, when it is fixed.
    pub fn fixed_min(&self) -> Option<u32> {
        match self.min {
            Amount::Fixed(min) => Some(u32::try_from(min).unwrap_or(0)),
            _ => None,
        }
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
            min: Amount::one(),
            max: Some(Amount::one()),
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

    #[rstest]
    #[case::any_cast(Pred::WasCast { by: None, from: None }, r#"{"WasCast":{}}"#)]
    #[case::cast_by_you_from_hand(
        Pred::WasCast { by: Some(Ref::You), from: Some(ZoneKind::Hand) },
        r#"{"WasCast":{"by":"You","from":"Hand"}}"#
    )]
    #[case::cast_with(Pred::CastWith(CastWay::Flashback), r#"{"CastWith":"Flashback"}"#)]
    #[case::top(Pred::TopOfLibrary, r#""TopOfLibrary""#)]
    #[case::bottom(Pred::BottomOfLibrary, r#""BottomOfLibrary""#)]
    #[case::blocked_by(Pred::BlockedBy(Ref::This), r#"{"BlockedBy":"This"}"#)]
    #[case::blocks(Pred::Blocks(Ref::This), r#"{"Blocks":"This"}"#)]
    #[case::targeted_by(Pred::TargetedBy(Ref::Triggering), r#"{"TargetedBy":"Triggering"}"#)]
    #[case::greatest(
        Pred::GreatestAmong { stat: StatKind::Power, group: Selector(vec![Pred::Other]) },
        r#"{"GreatestAmong":{"stat":"Power","group":["Other"]}}"#
    )]
    #[case::suspended(Pred::Suspended, r#""Suspended""#)]
    #[case::kicked(Pred::Kicked, r#""Kicked""#)]
    fn the_cast_library_combat_and_greatest_atoms_have_stable_json(
        #[case] value: Pred,
        #[case] json: &str,
    ) {
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
            bounds: Bounds::up_to(None),
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
