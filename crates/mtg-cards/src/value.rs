//! Values: amounts, conditions and the small enums they use. They are evaluated, never run.

use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use mtg_core::color::Color;
use mtg_core::mana::ManaType;

use crate::cost::Cost;
use crate::refs::{Ref, Selector};

/// A characteristic of an object that has a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatKind {
    /// Power.
    Power,
    /// Toughness.
    Toughness,
    /// Mana value.
    ManaValue,
    /// Loyalty.
    Loyalty,
    /// How many colors it has (CR 105.4): none for a colorless object.
    Colors,
}

/// A comparison between two numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cmp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
}

impl Cmp {
    /// Whether `left` compares to `right` this way.
    pub const fn holds(self, left: i64, right: i64) -> bool {
        match self {
            Cmp::Eq => left == right,
            Cmp::Ne => left != right,
            Cmp::Lt => left < right,
            Cmp::Le => left <= right,
            Cmp::Gt => left > right,
            Cmp::Ge => left >= right,
        }
    }
}

/// The alternative cost a spell is cast for (CR 118.9): what a permanent and its abilities remember of how it was cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum CastWay {
    /// Flashback (CR 702.34): from a graveyard, exiled when it leaves the stack.
    Flashback,
    /// Evoke (CR 702.74): sacrificed as it enters.
    Evoke,
    /// Overload (CR 702.96): every "target" reads "each".
    Overload,
    /// Madness (CR 702.35): from exile after it was discarded.
    Madness,
    /// Miracle (CR 702.94): from a hand for its miracle cost when it is the first card its owner drew this turn.
    Miracle,
    /// Suspend (CR 702.62): cast without paying its mana cost as the last time counter leaves; a creature has haste.
    Suspend,
    /// Foretell (CR 702.143): from exile, face down, on a later turn than it was foretold, for its foretell cost.
    Foretold,
    /// Escape (CR 702.138): from a graveyard for its escape cost, which exiles other cards of the graveyard; the permanent remembers that it escaped.
    Escape,
    /// Dash (CR 702.109): the creature has haste and returns to its owner's hand at the next end step.
    Dash,
    /// Bestow (CR 702.103): cast as an Aura spell with enchant creature; the permanent is an Aura while it is attached.
    Bestow,
    /// Blitz (CR 702.152): the creature has haste and "When this creature dies, draw a card" and is sacrificed at the next end step.
    Blitz,
    /// Prowl (CR 702.76): a spell cast for its prowl cost, which its abilities read as "if its prowl cost was paid".
    Prowl,
    /// Spectacle (CR 702.137): a spell cast from a hand for its spectacle cost, which its abilities read as "if its spectacle cost was paid".
    Spectacle,
    /// Mayhem (CR 702.190): a spell cast from a graveyard for its mayhem cost, which its abilities read as "if its mayhem cost was paid".
    Mayhem,
    /// Awaken (CR 702.113): a spell cast from a hand for its awaken cost, which also puts counters on a land and animates it.
    Awaken,
    /// Jump-start (CR 702.133): from a graveyard for its mana cost and the discard of a card; exiled when it leaves the stack.
    JumpStart,
    /// Retrace (CR 702.81): from a graveyard for its mana cost and the discard of a land card.
    Retrace,
    /// Morph, megamorph and disguise (CR 702.37c, 702.168): from a hand face down as a 2/2 creature spell for {3}; the permanent lies face down until it is turned face up.
    FaceDown,
    /// Mutate (CR 702.140): a creature spell cast from a hand for its mutate cost that targets a non-Human creature with the same owner and merges with it as it resolves.
    Mutate,
    /// Without paying its mana cost, by a permission or an effect (CR 118.9).
    Free,
}

impl CastWay {
    /// Whether the way pays another cost in place of the mana cost (CR 118.9); jump-start and retrace add a cost to it.
    pub const fn replaces_cost(self) -> bool {
        !matches!(self, CastWay::JumpStart | CastWay::Retrace)
    }

    /// Whether a spell cast this way is exiled when it leaves the stack (flashback, jump-start).
    pub const fn exiles_the_spell(self) -> bool {
        matches!(self, CastWay::Flashback | CastWay::JumpStart)
    }
}

/// A kind of counter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum CounterKind {
    /// A +1/+1 counter.
    #[serde(rename = "+1/+1")]
    PlusOnePlusOne,
    /// A -1/-1 counter.
    #[serde(rename = "-1/-1")]
    MinusOneMinusOne,
    /// A loyalty counter.
    #[serde(rename = "loyalty")]
    Loyalty,
    /// A poison counter.
    #[serde(rename = "poison")]
    Poison,
    /// A charge counter.
    #[serde(rename = "charge")]
    Charge,
    /// Any other counter, by its lowercase name.
    Other(String),
}

impl CounterKind {
    /// The lore counter of a Saga (CR 714).
    pub fn lore() -> Self {
        CounterKind::Other("lore".to_owned())
    }

    /// The kind as a card names it: `+1/+1`, `poison`, `charge`.
    pub fn name(&self) -> &str {
        match self {
            CounterKind::PlusOnePlusOne => "+1/+1",
            CounterKind::MinusOneMinusOne => "-1/-1",
            CounterKind::Loyalty => "loyalty",
            CounterKind::Poison => "poison",
            CounterKind::Charge => "charge",
            CounterKind::Other(name) => name,
        }
    }
}

/// What the game counts for a player in a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TurnTally {
    /// The creatures the player attacked with, each declared attacker once (CR 508.1).
    Attackers,
    /// The damage dealt to the player after prevention, by any source.
    DamageTaken,
    /// The times the player gained life, once for each gain of any amount.
    LifeGained,
    /// The times counters were put on the object (read through an object reference, not a player): one for each
    /// counter kind put, whatever the number ("a counter was put on it this turn").
    CountersPut,
    /// The life the player gained, by amount (`LifeGained` counts the gains). Summed over the players named.
    LifeGainedTotal,
    /// The creatures that attacked this turn, whoever controls them. Summed over the players named.
    CreaturesAttacked,
    /// The spells the player cast this turn: while a spell is being cast the one being cast is not counted, so "the
    /// second spell you cast each turn" is a count of one.
    SpellsCast,
    /// The life the players lost, added up over the players read ("for each 1 life your opponents have lost this turn"):
    /// a loss, a payment or damage that takes life; infect damage does not.
    LifeLost,
    /// The creatures put into a graveyard from the battlefield, tokens included, added up over the owners read ("the number
    /// of creatures that died this turn": `EachPlayer`; "for each creature put into your graveyard from the battlefield
    /// this turn": `You`).
    CreaturesDied,
    /// The same without the tokens ("for each nontoken creature put into your graveyard from the battlefield this turn").
    NontokenCreaturesDied,
}

/// A number, fixed or worked out when the effect runs.
///
/// A fixed amount is a bare JSON number; the other forms are serde's usual tagged shape.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Amount {
    /// A fixed number.
    Fixed(i32),
    /// The X announced for the spell or ability (CR 107.3).
    X,
    /// How many objects match the selector when the effect runs.
    Count(Selector),
    /// A characteristic of the referenced object (its last known value if it left).
    Stat {
        /// The object.
        of: Ref,
        /// The characteristic.
        stat: StatKind,
    },
    /// A player's life total.
    Life(Ref),
    /// How many times the player has cast their commanders from the command zone this game, all of them together
    /// (CR 903.8): the count behind the commander tax.
    CommanderCasts(Ref),
    /// How many counters of a kind are on the object or player.
    Counters {
        /// The object or player.
        on: Ref,
        /// The kind of counter.
        counter: CounterKind,
    },
    /// The sum of the amounts (negative numbers subtract).
    Sum(Vec<Amount>),
    /// The amount times a fixed factor, which may be negative: "gain 2 life for each creature you control" is the
    /// factor 2 of a `Count`.
    Times {
        /// The factor.
        by: i32,
        /// The amount multiplied.
        of: Box<Amount>,
    },
    /// This target's share of the amount divided as the controller chose.
    Divided,
    /// The amount divided evenly, rounded down, among the targets the effect names.
    EvenShare(Box<Amount>),
    /// How many targets were chosen for the target specification with this index.
    TargetCount(u8),
    /// The controller's devotion to the colors: the mana symbols of those colors among the mana costs of the
    /// permanents they control (CR 700.5).
    Devotion(Vec<Color>),
    /// The greatest value of the characteristic among the objects that match the selector; zero when none match.
    Greatest {
        /// The objects compared.
        of: Selector,
        /// The characteristic.
        stat: StatKind,
    },
    /// The sum of the characteristic over the objects that match the selector.
    Total {
        /// The objects added up.
        of: Selector,
        /// The characteristic.
        stat: StatKind,
    },
    /// How many card types there are among the objects that match the selector.
    CardTypes(Selector),
    /// How many players the reference names: `Players(Opponents)` is the number of opponents.
    Players(Ref),
    /// The lowest life total among the players the reference names (a player "has 13 or less life": `EachPlayer`; "an
    /// opponent has 10 or less life": `Opponents`). With no player named it is larger than any life total, so "at most"
    /// is false.
    LowestLife(Ref),
    /// The highest life total among the players the reference names ("each opponent has 10 or less life": `Opponents`).
    /// With no player named it is smaller than any life total, so "at most" is true of no one.
    HighestLife(Ref),
    /// How many times the kicker costs of the spell were paid, a multikicker once for each payment (CR 702.33); a
    /// permanent and its abilities read the spell it came from, and zero is no kicker paid.
    Kicks,
    /// How many times the permanents the reference names have mutated: the cards merged into each, less the one it was
    /// (CR 702.140; "the number of times this creature has mutated"); zero for a permanent that never merged.
    Mutations(Ref),
    /// One if the caster promised a gift as they cast the spell the ability reads, else zero (CR 702.174).
    Promised,
    /// One if the spell was cast for this alternative cost, else zero (a target specification that only awaken adds reads it in its most).
    CastWith(CastWay),
    /// How many colors there are among the mana spent to cast the spell the source is or came from (CR 702.44, sunburst).
    ColorsSpent,
    /// How many permanents were put into a graveyard from the battlefield this turn, tokens included (CR 702.69, gravestorm).
    PermanentsGraved,
    /// The largest count of the kind among the players the reference names this turn ("you attacked with three or more
    /// creatures this turn": `You`; "an opponent was dealt 7 or more damage this turn": `Opponents`).
    Tallied {
        /// The players read.
        who: Ref,
        /// What is counted.
        what: TurnTally,
    },
    /// The largest count of the objects the selector names among the players the reference names, each player taken as
    /// "you" (`[Land, ControlledBy You]` is the lands that player controls): "an opponent controls seven or more
    /// lands" compares it with `Opponents`.
    Largest {
        /// The players compared.
        who: Ref,
        /// The objects counted for each, written for "you".
        of: Selector,
    },
    /// The life total the players the reference names began the game with (the least of them).
    StartingLife(Ref),
    /// The amount of the event that triggered the ability ("that much", "that many", "the damage dealt"): the damage
    /// dealt, the life gained or lost, the counters put or removed; for a batch ("one or more") how many of its events
    /// the pattern found.
    Triggering,
}

impl Amount {
    /// The default amount: one.
    pub const fn one() -> Amount {
        Amount::Fixed(1)
    }

    /// Whether this is the fixed amount one, so serialization can leave it out.
    pub const fn is_one(&self) -> bool {
        matches!(self, Amount::Fixed(1))
    }

    /// The number of a fixed amount; `None` for any other.
    pub fn fixed(&self) -> Option<u32> {
        match self {
            Amount::Fixed(number) => u32::try_from(*number).ok(),
            _ => None,
        }
    }
}

/// The tagged forms of [`Amount`], read by serde so errors keep their path.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
enum Tagged {
    X,
    Count(Selector),
    Stat { of: Ref, stat: StatKind },
    Life(Ref),
    CommanderCasts(Ref),
    Counters { on: Ref, counter: CounterKind },
    Sum(Vec<Amount>),
    Times { by: i32, of: Box<Amount> },
    Divided,
    EvenShare(Box<Amount>),
    TargetCount(u8),
    Devotion(Vec<Color>),
    Greatest { of: Selector, stat: StatKind },
    Total { of: Selector, stat: StatKind },
    CardTypes(Selector),
    Players(Ref),
    LowestLife(Ref),
    HighestLife(Ref),
    Kicks,
    Mutations(Ref),
    Promised,
    CastWith(CastWay),
    ColorsSpent,
    PermanentsGraved,
    Tallied { who: Ref, what: TurnTally },
    Largest { who: Ref, of: Selector },
    StartingLife(Ref),
    Triggering,
}

/// The tagged forms of [`Amount`] by reference, written by serde.
#[derive(Serialize)]
enum TaggedRef<'a> {
    X,
    Count(&'a Selector),
    Stat {
        of: &'a Ref,
        stat: StatKind,
    },
    Life(&'a Ref),
    CommanderCasts(&'a Ref),
    Counters {
        on: &'a Ref,
        counter: &'a CounterKind,
    },
    Sum(&'a [Amount]),
    Times {
        by: i32,
        of: &'a Amount,
    },
    Divided,
    EvenShare(&'a Amount),
    TargetCount(u8),
    Devotion(&'a [Color]),
    Greatest {
        of: &'a Selector,
        stat: StatKind,
    },
    Total {
        of: &'a Selector,
        stat: StatKind,
    },
    CardTypes(&'a Selector),
    Players(&'a Ref),
    LowestLife(&'a Ref),
    HighestLife(&'a Ref),
    Kicks,
    Mutations(&'a Ref),
    Promised,
    CastWith(CastWay),
    ColorsSpent,
    PermanentsGraved,
    Tallied {
        who: &'a Ref,
        what: TurnTally,
    },
    Largest {
        who: &'a Ref,
        of: &'a Selector,
    },
    StartingLife(&'a Ref),
    Triggering,
}

impl From<Tagged> for Amount {
    fn from(tagged: Tagged) -> Amount {
        match tagged {
            Tagged::X => Amount::X,
            Tagged::Count(selector) => Amount::Count(selector),
            Tagged::Stat { of, stat } => Amount::Stat { of, stat },
            Tagged::Life(who) => Amount::Life(who),
            Tagged::CommanderCasts(who) => Amount::CommanderCasts(who),
            Tagged::Counters { on, counter } => Amount::Counters { on, counter },
            Tagged::Sum(parts) => Amount::Sum(parts),
            Tagged::Times { by, of } => Amount::Times { by, of },
            Tagged::Divided => Amount::Divided,
            Tagged::EvenShare(total) => Amount::EvenShare(total),
            Tagged::TargetCount(index) => Amount::TargetCount(index),
            Tagged::Devotion(colors) => Amount::Devotion(colors),
            Tagged::Greatest { of, stat } => Amount::Greatest { of, stat },
            Tagged::Total { of, stat } => Amount::Total { of, stat },
            Tagged::CardTypes(of) => Amount::CardTypes(of),
            Tagged::Players(who) => Amount::Players(who),
            Tagged::LowestLife(who) => Amount::LowestLife(who),
            Tagged::HighestLife(who) => Amount::HighestLife(who),
            Tagged::Kicks => Amount::Kicks,
            Tagged::Mutations(of) => Amount::Mutations(of),
            Tagged::Promised => Amount::Promised,
            Tagged::CastWith(way) => Amount::CastWith(way),
            Tagged::ColorsSpent => Amount::ColorsSpent,
            Tagged::PermanentsGraved => Amount::PermanentsGraved,
            Tagged::Tallied { who, what } => Amount::Tallied { who, what },
            Tagged::Largest { who, of } => Amount::Largest { who, of },
            Tagged::StartingLife(who) => Amount::StartingLife(who),
            Tagged::Triggering => Amount::Triggering,
        }
    }
}

impl Serialize for Amount {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Amount::Fixed(n) => serializer.serialize_i32(*n),
            Amount::X => TaggedRef::X.serialize(serializer),
            Amount::Count(selector) => TaggedRef::Count(selector).serialize(serializer),
            Amount::Stat { of, stat } => TaggedRef::Stat { of, stat: *stat }.serialize(serializer),
            Amount::Life(who) => TaggedRef::Life(who).serialize(serializer),
            Amount::CommanderCasts(who) => TaggedRef::CommanderCasts(who).serialize(serializer),
            Amount::Counters { on, counter } => {
                TaggedRef::Counters { on, counter }.serialize(serializer)
            }
            Amount::Sum(parts) => TaggedRef::Sum(parts).serialize(serializer),
            Amount::Times { by, of } => TaggedRef::Times { by: *by, of }.serialize(serializer),
            Amount::Divided => TaggedRef::Divided.serialize(serializer),
            Amount::EvenShare(total) => TaggedRef::EvenShare(total).serialize(serializer),
            Amount::TargetCount(index) => TaggedRef::TargetCount(*index).serialize(serializer),
            Amount::Devotion(colors) => TaggedRef::Devotion(colors).serialize(serializer),
            Amount::Greatest { of, stat } => {
                TaggedRef::Greatest { of, stat: *stat }.serialize(serializer)
            }
            Amount::Total { of, stat } => {
                TaggedRef::Total { of, stat: *stat }.serialize(serializer)
            }
            Amount::CardTypes(of) => TaggedRef::CardTypes(of).serialize(serializer),
            Amount::Players(who) => TaggedRef::Players(who).serialize(serializer),
            Amount::LowestLife(who) => TaggedRef::LowestLife(who).serialize(serializer),
            Amount::HighestLife(who) => TaggedRef::HighestLife(who).serialize(serializer),
            Amount::Kicks => TaggedRef::Kicks.serialize(serializer),
            Amount::Mutations(of) => TaggedRef::Mutations(of).serialize(serializer),
            Amount::Promised => TaggedRef::Promised.serialize(serializer),
            Amount::CastWith(way) => TaggedRef::CastWith(*way).serialize(serializer),
            Amount::ColorsSpent => TaggedRef::ColorsSpent.serialize(serializer),
            Amount::PermanentsGraved => TaggedRef::PermanentsGraved.serialize(serializer),
            Amount::Tallied { who, what } => {
                TaggedRef::Tallied { who, what: *what }.serialize(serializer)
            }
            Amount::Largest { who, of } => TaggedRef::Largest { who, of }.serialize(serializer),
            Amount::StartingLife(who) => TaggedRef::StartingLife(who).serialize(serializer),
            Amount::Triggering => TaggedRef::Triggering.serialize(serializer),
        }
    }
}

struct AmountVisitor;

impl<'de> Visitor<'de> for AmountVisitor {
    type Value = Amount;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a number, \"X\", \"Divided\", or a one-key map such as {\"Count\": [...]}")
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Amount, E> {
        i32::try_from(value)
            .map(Amount::Fixed)
            .map_err(|_| E::custom(format!("amount {value} is out of range")))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Amount, E> {
        i32::try_from(value)
            .map(Amount::Fixed)
            .map_err(|_| E::custom(format!("amount {value} is out of range")))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Amount, E> {
        Tagged::deserialize(de::value::StrDeserializer::<E>::new(value)).map(Amount::from)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Amount, A::Error> {
        Tagged::deserialize(de::value::MapAccessDeserializer::new(map)).map(Amount::from)
    }
}

impl<'de> Deserialize<'de> for Amount {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Amount, D::Error> {
        deserializer.deserialize_any(AmountVisitor)
    }
}

/// A fact about the game, true or false when it is checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Condition {
    /// The condition is false.
    Not(Box<Condition>),
    /// Every condition is true.
    All(Vec<Condition>),
    /// At least one condition is true.
    Any(Vec<Condition>),
    /// Two amounts compare this way.
    Compare {
        /// The left amount.
        left: Amount,
        /// The comparison.
        op: Cmp,
        /// The right amount.
        right: Amount,
    },
    /// The referenced object has the selector's characteristics (its zone is ignored).
    Matches {
        /// The object tested.
        subject: Ref,
        /// The predicates it must satisfy.
        selector: Selector,
    },
    /// The spell was kicked (any kicker cost was paid); a permanent and its abilities read the spell it came from.
    Kicked,
    /// The spell was kicked with this one of its kicker costs ('Kicker {W} and/or {2}{B}'): "if it was kicked with its {W}
    /// kicker"; a permanent and its abilities read the spell it came from.
    KickedWith(Cost),
    /// The optional additional cost of the spell ("As an additional cost to cast this spell, you may ...") was paid; a
    /// permanent and its abilities read the spell it came from.
    AdditionalCostPaid,
    /// The spell was cast for this alternative cost; a permanent and its abilities read the spell it came from.
    CastWith(CastWay),
    /// At least one mana of this type was spent to cast the spell (CR 601.2h): "if {R} was spent to cast this spell"; a
    /// permanent and its abilities read the spell it came from, as the mana it was cast with.
    ManaSpent(ManaType),
    /// It is the controller's turn.
    YourTurn,
    /// The effect just before this one in its list was carried out in full ("if you do"): a "may" that was
    /// accepted, a cost that was paid, cards that were moved.
    Performed,
    /// A player the reference names is the monarch (CR 724): any of them when it names several, so `EachPlayer` is "there is a
    /// monarch" and its `Not` "there is no monarch".
    Monarch(Ref),
    /// A player the reference names was the monarch as the turn began: the designation they held before anything in the
    /// turn changed it ("if you were the monarch as the turn began").
    MonarchAtTurnStart(Ref),
    /// The player has the city's blessing (CR 702.131).
    CitysBlessing(Ref),
    /// The referenced permanents are controlled by the controller and came under their control since their last turn began (echo, CR 702.30a): read in the upkeep, once that turn has begun.
    NewlyControlled(Ref),
    /// The referenced Class compares to the level this way (CR 716.2): a Class that has gained no level is level 1.
    Level {
        /// The Class read; `This` for the Class with the ability.
        of: Ref,
        /// How its level compares to `level`.
        op: Cmp,
        /// The level compared to.
        level: u32,
    },
}

#[cfg(test)]
mod tests {
    //! Goal: amounts are bare numbers when fixed and keep serde's tagged shape otherwise, with errors that say what was expected; of the ways to cast, only jump-start and retrace keep the mana cost.
    use super::*;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use crate::refs::Pred;

    #[rstest]
    #[case::flashback(CastWay::Flashback, true)]
    #[case::evoke(CastWay::Evoke, true)]
    #[case::overload(CastWay::Overload, true)]
    #[case::madness(CastWay::Madness, true)]
    #[case::suspend(CastWay::Suspend, true)]
    #[case::foretold(CastWay::Foretold, true)]
    #[case::escape(CastWay::Escape, true)]
    #[case::dash(CastWay::Dash, true)]
    #[case::blitz(CastWay::Blitz, true)]
    #[case::prowl(CastWay::Prowl, true)]
    #[case::spectacle(CastWay::Spectacle, true)]
    #[case::mayhem(CastWay::Mayhem, true)]
    #[case::mutate(CastWay::Mutate, true)]
    #[case::face_down(CastWay::FaceDown, true)]
    #[case::free(CastWay::Free, true)]
    #[case::jump_start(CastWay::JumpStart, false)]
    #[case::retrace(CastWay::Retrace, false)]
    fn only_a_way_that_adds_a_cost_keeps_the_mana_cost(
        #[case] way: CastWay,
        #[case] replaces: bool,
    ) {
        assert_eq!(way.replaces_cost(), replaces);
    }

    #[rstest]
    #[case::fixed(Amount::Fixed(3), "3")]
    #[case::negative(Amount::Fixed(-1), "-1")]
    #[case::x(Amount::X, r#""X""#)]
    #[case::divided(Amount::Divided, r#""Divided""#)]
    #[case::count(
        Amount::Count(Selector(vec![Pred::Type(CardType::Creature)])),
        r#"{"Count":[{"Type":"Creature"}]}"#
    )]
    #[case::stat(
        Amount::Stat { of: Ref::Target(0), stat: StatKind::Power },
        r#"{"Stat":{"of":{"Target":0},"stat":"Power"}}"#
    )]
    #[case::sum(
        Amount::Sum(vec![Amount::TargetCount(0), Amount::Fixed(-1)]),
        r#"{"Sum":[{"TargetCount":0},-1]}"#
    )]
    #[case::times(
        Amount::Times { by: -1, of: Box::new(Amount::Players(Ref::Opponents)) },
        r#"{"Times":{"by":-1,"of":{"Players":"Opponents"}}}"#
    )]
    #[case::even(Amount::EvenShare(Box::new(Amount::X)), r#"{"EvenShare":"X"}"#)]
    #[case::opponents(Amount::Players(Ref::Opponents), r#"{"Players":"Opponents"}"#)]
    #[case::lowest_life(Amount::LowestLife(Ref::EachPlayer), r#"{"LowestLife":"EachPlayer"}"#)]
    #[case::highest_life(Amount::HighestLife(Ref::Opponents), r#"{"HighestLife":"Opponents"}"#)]
    #[case::counters(
        Amount::Counters { on: Ref::This, counter: CounterKind::PlusOnePlusOne },
        r#"{"Counters":{"on":"This","counter":"+1/+1"}}"#
    )]
    fn amounts_have_stable_json(#[case] value: Amount, #[case] json: &str) {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(serde_json::from_str::<Amount>(json).unwrap(), value);
    }

    #[rstest]
    #[case::float("1.5", "expected a number")]
    #[case::unknown_word(r#""Y""#, "unknown variant `Y`")]
    #[case::two_keys(r#"{"Life":"You","extra":1}"#, "line 1 column")]
    #[case::too_big("99999999999", "out of range")]
    fn bad_amounts_say_why(#[case] json: &str, #[case] expected: &str) {
        let error = serde_json::from_str::<Amount>(json)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }

    #[test]
    fn comparisons_hold_as_written() {
        assert!(Cmp::Ge.holds(40, 40));
        assert!(!Cmp::Lt.holds(2, 2));
        assert!(Cmp::Ne.holds(1, 2));
    }
}
