//! Values: amounts, conditions and the small enums they use. They are evaluated, never run.

use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use mtg_core::color::Color;
use mtg_core::mana::ManaType;

use crate::cost::Cost;
use crate::keyword::Keyword;
use crate::refs::{Ref, Selector, ZoneKind};
use crate::trigger::Step;

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
    /// Its basic land types (CR 305.6): how many as a number, each type once among objects (`Amount::Distinct`, domain).
    BasicLandTypes,
    /// Its printed power, as the card has it before any effect (a face-down or copied object reads what it is now).
    BasePower,
    /// Its printed toughness, as the card has it before any effect.
    BaseToughness,
    /// The mana spent to cast it, while it is a spell or when it was cast.
    ManaSpent,
    /// Its name: no number, so only `Amount::Distinct` reads it (the different names among the objects).
    Name,
    /// Its creature types: no number, so only `Amount::MostCommon` reads it (the most creatures sharing one type).
    CreatureType,
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
    /// Plot (CR 702.170): from exile, face up, as a sorcery on a later turn than it was plotted, without paying its mana cost.
    Plotted,
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
    /// Disturb (CR 702.146): the back face of a transforming double-faced card cast from a graveyard for its disturb cost, which its abilities read as "if its disturb cost was paid"; exiled when it leaves the stack.
    Disturb,
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
    /// Warp (the alternative cost table): a card cast from a hand for its warp cost, which its abilities read as "if its warp cost was paid".
    Warp,
    /// Surge: a spell cast from a hand for its surge cost after the caster cast another spell this turn.
    Surge,
    /// Web-slinging: a spell cast from a hand for its web-slinging cost, which returns a tapped creature the caster controls.
    WebSlinging,
    /// Impending: a permanent spell cast from a hand for its impending cost; it enters with time counters and is no creature while it has one.
    Impending,
    /// Without paying its mana cost, by a permission or an effect (CR 118.9).
    Free,
    /// Emerge (CR 702.119): from a hand for its emerge cost and the sacrifice of a creature, reduced by that creature's mana value.
    Emerge,
    /// Harmonize (CR 702.180): from a graveyard for its harmonize cost, reduced by the power of a creature tapped for it; exiled when it leaves the stack.
    Harmonize,
    /// An alternative cost the card prints in a static line, chosen per card (CR 118.9): cast from where `AltCast` names for the cost it gives.
    Alternative,
}

impl CastWay {
    /// Whether the way pays another cost in place of the mana cost (CR 118.9); jump-start and retrace add a cost to it.
    pub const fn replaces_cost(self) -> bool {
        !matches!(self, CastWay::JumpStart | CastWay::Retrace)
    }

    /// Whether a spell cast this way is exiled when it leaves the stack (flashback, jump-start, harmonize, disturb).
    pub const fn exiles_the_spell(self) -> bool {
        matches!(
            self,
            CastWay::Flashback | CastWay::JumpStart | CastWay::Harmonize | CastWay::Disturb
        )
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

    /// The defense counter of a battle (CR 310.4).
    pub fn defense() -> Self {
        CounterKind::Other("defense".to_owned())
    }

    /// The keyword a counter of this kind gives its permanent (CR 122.1b): a counter named for a keyword that adds no
    /// ability of its own. `exalted` and `decayed` counters are not among them.
    pub fn keyword(&self) -> Option<Keyword> {
        let CounterKind::Other(name) = self else {
            return None;
        };
        Some(match name.as_str() {
            "flying" => Keyword::Flying,
            "first strike" => Keyword::FirstStrike,
            "double strike" => Keyword::DoubleStrike,
            "deathtouch" => Keyword::Deathtouch,
            "haste" => Keyword::Haste,
            "hexproof" => Keyword::Hexproof,
            "indestructible" => Keyword::Indestructible,
            "lifelink" => Keyword::Lifelink,
            "menace" => Keyword::Menace,
            "reach" => Keyword::Reach,
            "shadow" => Keyword::Shadow,
            "skulk" => Keyword::Skulk,
            "trample" => Keyword::Trample,
            "vigilance" => Keyword::Vigilance,
            _ => return None,
        })
    }

    /// The power and toughness one counter of this kind adds (CR 122.1a, 613.4c): `+1/+1` adds (1, 1), `-1/-1` adds
    /// (-1, -1) and a counter named for its numbers such as `+2/+0` or `-0/-1` adds those; any other kind adds nothing.
    pub fn power_toughness(&self) -> (i32, i32) {
        match self {
            CounterKind::PlusOnePlusOne => (1, 1),
            CounterKind::MinusOneMinusOne => (-1, -1),
            CounterKind::Other(name) => name
                .split_once('/')
                .and_then(|(power, toughness)| Some((signed(power)?, signed(toughness)?)))
                .unwrap_or((0, 0)),
            _ => (0, 0),
        }
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

/// A counter's signed number written with its sign (`+2`, `-0`).
fn signed(text: &str) -> Option<i32> {
    let (sign, digits) = match text.split_at_checked(1)? {
        ("+", digits) => (1, digits),
        ("-", digits) => (-1, digits),
        _ => return None,
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<i32>().ok().map(|n| sign * n)
}

/// Whether it is day or night (CR 726.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum DayNight {
    /// It is day.
    Day,
    /// It is night.
    Night,
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
    /// The instant and sorcery spells the player cast this turn ("the number of instant and sorcery spells you've cast
    /// this turn").
    InstantsAndSorceriesCast,
    /// The noncreature spells the player cast this turn ("if you've cast a noncreature spell this turn").
    NoncreatureSpellsCast,
    /// The creature spells the player cast this turn ("the first creature spell you cast each turn" is a count of zero).
    CreatureSpellsCast,
    /// The artifact spells the player cast this turn ("the first artifact spell you cast each turn").
    ArtifactSpellsCast,
    /// The cards the player discarded this turn, once for each discard ("for each card you've discarded this turn").
    Discarded,
    /// The life the players lost, added up over the players read ("for each 1 life your opponents have lost this turn"):
    /// a loss, a payment or damage that takes life; infect damage does not.
    LifeLost,
    /// The creatures put into a graveyard from the battlefield, tokens included, added up over the owners read ("the number
    /// of creatures that died this turn": `EachPlayer`; "for each creature put into your graveyard from the battlefield
    /// this turn": `You`).
    CreaturesDied,
    /// The same without the tokens ("for each nontoken creature put into your graveyard from the battlefield this turn").
    NontokenCreaturesDied,
    /// The cards the player drew this turn ("for each card you've drawn this turn").
    Drawn,
    /// The lands the player played this turn: a land put onto the battlefield by an effect is not played (CR 305.1).
    LandsPlayed,
    /// The creatures that died under the control of the players read, tokens included: put into a graveyard from the
    /// battlefield while they controlled them ("a creature died under your control this turn"). Added up over the players.
    CreaturesDiedUnder,
    /// The same without the tokens ("for each nontoken creature that died under your control this turn").
    NontokenCreaturesDiedUnder,
    /// The Phyrexians put into a graveyard from the battlefield while the players read controlled them ("if a Phyrexian died
    /// under your control this turn"), tokens included. Added up over the players.
    PhyrexiansDiedUnder,
    /// The cards that left the graveyards of the players read this turn, to any zone ("if a card left your graveyard this
    /// turn"). Added up over the players.
    CardsLeftGraveyard,
    /// The permanents that left the battlefield for any zone while the players read controlled them (revolt: "if a
    /// permanent left the battlefield under your control this turn"). Added up over the players.
    PermanentsLeftUnder,
    /// The times the player scried this turn, once for each scry that looked at a card.
    Scried,
    /// The times the player surveilled this turn, once for each surveil that looked at a card.
    Surveilled,
    /// The permanents the player sacrificed this turn, each once, as the player who controlled it.
    Sacrificed,
    /// The dungeons the player has completed this game (CR 309.7), each completion once; not reset by the turn.
    DungeonsCompleted,
    /// The nonland permanents that left the battlefield for any zone while the players read controlled them (void).
    /// Added up over the players.
    NonlandPermanentsLeft,
    /// The spells the player cast with warp this turn (void). Added up over the players.
    SpellsWarped,
}

/// A kind of event the game logs for the turn with the object it was about, which `Amount::Tally` filters and counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TallyEvent {
    /// A token was created (the object is the token, as it entered).
    TokenCreated,
    /// A spell was cast (the object is the spell, as it was cast).
    SpellCast,
    /// A permanent entered the battlefield, a token included (the object as it entered, under the controller it entered with).
    Entered,
    /// A permanent was put into a graveyard from the battlefield (the object as it last existed there, tokens included).
    Died,
    /// Damage was dealt (the dealer; an amount counts the damage dealt, `Pred::DidThisTurn` reads who it went to).
    Damaged,
    /// A creature was declared as an attacker (the attacker; `Pred::DidThisTurn` reads who it attacks).
    Attacked,
    /// A creature was declared as a blocker (the blocker; `Pred::DidThisTurn` reads the attacker it blocks).
    Blocked,
    /// An ability was activated, a mana ability excepted (the source of the ability).
    Activated,
    /// Counters were put on a permanent or a card in exile by a spell or ability (the object; an amount counts the
    /// counters put, for the player who controlled the spell or ability).
    CountersPut,
}

/// Which result of the latest die roll an amount reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DieResult {
    /// The number a single die rolled ("the result").
    Only,
    /// The result the roller chose among several dice ("that result").
    Chosen,
    /// The result they did not choose ("the other result").
    Other,
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
    /// A characteristic of the referenced object (its last known value if it left); of several objects, their total.
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
    /// How many counters of any kind are on the object or player, all kinds together (CR 122.1).
    AnyCounters(Ref),
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
    /// Half of the amount, rounded up or down, never below zero ("half their life, rounded up").
    Half {
        /// The amount halved.
        of: Box<Amount>,
        /// Rounded up; else down.
        up: bool,
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
    /// How many different values of the characteristic there are among the objects that match the selector ("the number
    /// of different powers among creatures you control", "three or more creatures with different powers"); with
    /// `BasicLandTypes`, the basic land types among them, a land of several types counting each (domain).
    Distinct {
        /// The objects compared.
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
    /// The fewest cards in a hand among the players the reference names (a player "has one or fewer cards in hand":
    /// `EachPlayer`). With no player named it is larger than any hand, so "at most" is false.
    LowestHand(Ref),
    /// The life the players lost to the effects of the spell or ability that is resolving ("you gain life equal to the
    /// life lost this way"): the amounts of its `LifeLost` events so far.
    LifeLostThisWay,
    /// How many permanents the resolving spell or ability has destroyed so far that matched the selector as they last
    /// existed ("for each creature destroyed this way", "creatures you controlled that were destroyed this way"); tokens
    /// count, and a permanent regenerated or indestructible was not destroyed.
    Destroyed(Selector),
    /// How many permanents the resolving spell or ability has exiled from the battlefield so far that matched the selector
    /// as they last existed ("the number of creatures exiled this way"); tokens count.
    Exiled(Selector),
    /// How many permanents the resolving spell or ability has phased out so far that match the selector ("for each
    /// creature that phased out this way"); what phased out with them as attachments counts too.
    PhasedOut(Selector),
    /// How many times the kicker costs of the spell were paid, a multikicker once for each payment (CR 702.33); a
    /// permanent and its abilities read the spell it came from, and zero is no kicker paid.
    Kicks,
    /// How many times the permanents the reference names have mutated: the cards merged into each, less the one it was
    /// (CR 702.140; "the number of times this creature has mutated"); zero for a permanent that never merged.
    Mutations(Ref),
    /// One if the caster promised a gift as they cast the spell the ability reads, else zero (CR 702.174).
    Promised,
    /// How many times the ability that reads it has resolved this turn, this resolution included; zero outside a resolving
    /// ability (Ashling's "if this is the third time this ability has resolved this turn").
    ResolvedThisTurn,
    /// How many permanents this permanent devoured as it entered (CR 702.82); zero once it has left the battlefield.
    Devoured,
    /// The number a `Let` of the resolving spell or ability fixed under the name; zero before it ran.
    Bound(String),
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
    /// The most counters of the kind that any one of the players the reference names holds ("the most poison counters
    /// among your opponents"); never the sum, and zero when no player is named.
    HighestCounters {
        /// The players compared.
        who: Ref,
        /// The kind of counter.
        counter: CounterKind,
    },
    /// The life total the players the reference names began the game with (the least of them).
    StartingLife(Ref),
    /// The most spells one of the players the reference names cast during the previous turn ("no spells were cast last turn":
    /// `EachPlayer`, equal to 0; "a player cast two or more spells last turn": at least 2); 0 before any turn has ended.
    CastLastTurn(Ref),
    /// The size of the party of the player the reference names (CR 700.8): the most roles among Cleric, Rogue, Warrior and
    /// Wizard their creatures fill, each creature filling one role and each role once, so 0 to 4; the largest when it names several.
    Party(Ref),
    /// The amount of the event that triggered the ability ("that much", "that many", "the damage dealt"): the damage
    /// dealt, the life gained or lost, the counters put or removed; for a batch ("one or more") how many of its events
    /// the pattern found, and for a batch of damage the damage dealt in all.
    Triggering,
    /// The events of the kind this turn whose object the filter selects, counted for the players the reference names
    /// ("the second creature token you created this turn").
    Tally {
        /// The kind of event.
        event: TallyEvent,
        /// The players whose events are counted.
        who: Ref,
        /// The objects the events were about, as they were then; empty selects every one.
        filter: Selector,
    },
    /// The number of colors in the color identity of the commanders the players named have, all together (CR 903.4).
    CommanderColors(Ref),
    /// The numbers the sources named by `on` remember in `slot` (`Effect::RememberValue`), added up; 0 when none is held.
    Remembered {
        /// The object whose memory is read.
        on: Ref,
        /// The name of the memory.
        slot: String,
    },
    /// The number of the turn, counting from 1 (CR 500).
    TurnNumber,
    /// How many targets the spell or ability that triggered this one has.
    TriggeringTargets,
    /// A result of the latest die roll of the resolving spell or ability ("X, where X is the result", "equal to that result").
    DieResult(DieResult),
    /// The first amount when the condition holds when the effect runs, else the second ("X is 3 if you control a Goblin, else 1").
    When {
        /// The fact checked.
        condition: Box<Condition>,
        /// The amount when it holds.
        then: Box<Amount>,
        /// The amount when it does not.
        otherwise: Box<Amount>,
    },
    /// How much mana was spent to cast the spell the source is or came from: of the one type, or of all types when none is named.
    ManaSpent {
        /// The type counted; all mana when none.
        of: Option<ManaType>,
    },
    /// How many different colors of mana the players the reference names hold in their pools, all together.
    ManaColors(Ref),
    /// How many permanents share the value of the characteristic that most of the objects the selector names share
    /// ("the most common color among creatures you control", with `Colors` each color counted once per object); zero when none match.
    MostCommon {
        /// The objects compared.
        of: Selector,
        /// The characteristic.
        stat: StatKind,
    },
    /// A whole number from `min` to `max`, drawn from the game's seeded random stream, so a replay of the log draws it again.
    Random {
        /// The lowest number.
        min: i32,
        /// The highest number.
        max: i32,
    },
    /// How much mana the players the reference names hold in their pools, restricted mana included.
    ManaInPool(Ref),
    /// The doors unlocked among the Rooms the selector names (CR 709.5), both doors of a Room counting twice.
    Doors(Selector),
    /// The number the controller of the source chose as the source entered the battlefield; 0 when it chose none.
    Chosen,
    /// The number the resolution remembered under the name (`Effect::ChooseValue`); 0 for a value that is no number.
    ChosenAs(String),
    /// How many of the objects the reference names satisfy the filter, an object that left the zone judged as it was
    /// (CR 400.7): "the number of Humans among the sacrificed creatures". Zero when the reference names none.
    CountOf {
        /// The objects counted from.
        of: Ref,
        /// What each must satisfy.
        filter: Selector,
    },
    /// The product of the amounts: "X times the number of ...".
    Product(Vec<Amount>),
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

    /// The amount of nothing: zero.
    pub const fn zero() -> Amount {
        Amount::Fixed(0)
    }

    /// Whether this is the fixed amount zero, so serialization can leave it out.
    pub const fn is_zero(&self) -> bool {
        matches!(self, Amount::Fixed(0))
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
    Stat {
        of: Ref,
        stat: StatKind,
    },
    Life(Ref),
    CommanderCasts(Ref),
    Counters {
        on: Ref,
        counter: CounterKind,
    },
    AnyCounters(Ref),
    Sum(Vec<Amount>),
    Times {
        by: i32,
        of: Box<Amount>,
    },
    Half {
        of: Box<Amount>,
        up: bool,
    },
    Divided,
    EvenShare(Box<Amount>),
    TargetCount(u8),
    Devotion(Vec<Color>),
    Greatest {
        of: Selector,
        stat: StatKind,
    },
    Total {
        of: Selector,
        stat: StatKind,
    },
    Distinct {
        of: Selector,
        stat: StatKind,
    },
    CardTypes(Selector),
    Players(Ref),
    LowestLife(Ref),
    HighestLife(Ref),
    LowestHand(Ref),
    LifeLostThisWay,
    Destroyed(Selector),
    Exiled(Selector),
    PhasedOut(Selector),
    Kicks,
    Mutations(Ref),
    Promised,
    ResolvedThisTurn,
    Devoured,
    Bound(String),
    CastWith(CastWay),
    ColorsSpent,
    PermanentsGraved,
    Tallied {
        who: Ref,
        what: TurnTally,
    },
    Largest {
        who: Ref,
        of: Selector,
    },
    HighestCounters {
        who: Ref,
        counter: CounterKind,
    },
    StartingLife(Ref),
    CastLastTurn(Ref),
    Party(Ref),
    Triggering,
    Tally {
        event: TallyEvent,
        who: Ref,
        filter: Selector,
    },
    Remembered {
        on: Ref,
        slot: String,
    },
    CommanderColors(Ref),
    TurnNumber,
    TriggeringTargets,
    DieResult(DieResult),
    When {
        condition: Box<Condition>,
        then: Box<Amount>,
        otherwise: Box<Amount>,
    },
    ManaSpent {
        of: Option<ManaType>,
    },
    ManaColors(Ref),
    MostCommon {
        of: Selector,
        stat: StatKind,
    },
    Random {
        min: i32,
        max: i32,
    },
    ManaInPool(Ref),
    Doors(Selector),
    Chosen,
    ChosenAs(String),
    CountOf {
        of: Ref,
        filter: Selector,
    },
    Product(Vec<Amount>),
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
    AnyCounters(&'a Ref),
    Sum(&'a [Amount]),
    Times {
        by: i32,
        of: &'a Amount,
    },
    Half {
        of: &'a Amount,
        up: bool,
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
    Distinct {
        of: &'a Selector,
        stat: StatKind,
    },
    CardTypes(&'a Selector),
    Players(&'a Ref),
    LowestLife(&'a Ref),
    HighestLife(&'a Ref),
    LowestHand(&'a Ref),
    LifeLostThisWay,
    Destroyed(&'a Selector),
    Exiled(&'a Selector),
    PhasedOut(&'a Selector),
    Kicks,
    Mutations(&'a Ref),
    Promised,
    ResolvedThisTurn,
    Devoured,
    Bound(&'a str),
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
    HighestCounters {
        who: &'a Ref,
        counter: &'a CounterKind,
    },
    StartingLife(&'a Ref),
    CastLastTurn(&'a Ref),
    Party(&'a Ref),
    Triggering,
    Tally {
        event: TallyEvent,
        who: &'a Ref,
        filter: &'a Selector,
    },
    Remembered {
        on: &'a Ref,
        slot: &'a str,
    },
    CommanderColors(&'a Ref),
    TurnNumber,
    TriggeringTargets,
    DieResult(DieResult),
    When {
        condition: &'a Condition,
        then: &'a Amount,
        otherwise: &'a Amount,
    },
    ManaSpent {
        of: Option<ManaType>,
    },
    ManaColors(&'a Ref),
    MostCommon {
        of: &'a Selector,
        stat: StatKind,
    },
    Random {
        min: i32,
        max: i32,
    },
    ManaInPool(&'a Ref),
    Doors(&'a Selector),
    Chosen,
    ChosenAs(&'a String),
    CountOf {
        of: &'a Ref,
        filter: &'a Selector,
    },
    Product(&'a [Amount]),
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
            Tagged::AnyCounters(on) => Amount::AnyCounters(on),
            Tagged::Sum(parts) => Amount::Sum(parts),
            Tagged::Times { by, of } => Amount::Times { by, of },
            Tagged::Half { of, up } => Amount::Half { of, up },
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
            Tagged::LowestHand(who) => Amount::LowestHand(who),
            Tagged::LifeLostThisWay => Amount::LifeLostThisWay,
            Tagged::Distinct { of, stat } => Amount::Distinct { of, stat },
            Tagged::Destroyed(of) => Amount::Destroyed(of),
            Tagged::Exiled(of) => Amount::Exiled(of),
            Tagged::PhasedOut(of) => Amount::PhasedOut(of),
            Tagged::Kicks => Amount::Kicks,
            Tagged::Mutations(of) => Amount::Mutations(of),
            Tagged::Promised => Amount::Promised,
            Tagged::ResolvedThisTurn => Amount::ResolvedThisTurn,
            Tagged::Devoured => Amount::Devoured,
            Tagged::Bound(name) => Amount::Bound(name),
            Tagged::CastWith(way) => Amount::CastWith(way),
            Tagged::ColorsSpent => Amount::ColorsSpent,
            Tagged::PermanentsGraved => Amount::PermanentsGraved,
            Tagged::Tallied { who, what } => Amount::Tallied { who, what },
            Tagged::Largest { who, of } => Amount::Largest { who, of },
            Tagged::HighestCounters { who, counter } => Amount::HighestCounters { who, counter },
            Tagged::StartingLife(who) => Amount::StartingLife(who),
            Tagged::CastLastTurn(who) => Amount::CastLastTurn(who),
            Tagged::Party(who) => Amount::Party(who),
            Tagged::Triggering => Amount::Triggering,
            Tagged::DieResult(which) => Amount::DieResult(which),
            Tagged::Tally { event, who, filter } => Amount::Tally { event, who, filter },
            Tagged::Remembered { on, slot } => Amount::Remembered { on, slot },
            Tagged::CommanderColors(who) => Amount::CommanderColors(who),
            Tagged::TurnNumber => Amount::TurnNumber,
            Tagged::TriggeringTargets => Amount::TriggeringTargets,
            Tagged::When {
                condition,
                then,
                otherwise,
            } => Amount::When {
                condition,
                then,
                otherwise,
            },
            Tagged::ManaSpent { of } => Amount::ManaSpent { of },
            Tagged::ManaColors(who) => Amount::ManaColors(who),
            Tagged::MostCommon { of, stat } => Amount::MostCommon { of, stat },
            Tagged::Random { min, max } => Amount::Random { min, max },
            Tagged::ManaInPool(who) => Amount::ManaInPool(who),
            Tagged::Doors(of) => Amount::Doors(of),
            Tagged::Chosen => Amount::Chosen,
            Tagged::ChosenAs(name) => Amount::ChosenAs(name),
            Tagged::CountOf { of, filter } => Amount::CountOf { of, filter },
            Tagged::Product(parts) => Amount::Product(parts),
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
            Amount::AnyCounters(on) => TaggedRef::AnyCounters(on).serialize(serializer),
            Amount::Sum(parts) => TaggedRef::Sum(parts).serialize(serializer),
            Amount::Times { by, of } => TaggedRef::Times { by: *by, of }.serialize(serializer),
            Amount::Half { of, up } => TaggedRef::Half { of, up: *up }.serialize(serializer),
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
            Amount::LowestHand(who) => TaggedRef::LowestHand(who).serialize(serializer),
            Amount::LifeLostThisWay => TaggedRef::LifeLostThisWay.serialize(serializer),
            Amount::Distinct { of, stat } => {
                TaggedRef::Distinct { of, stat: *stat }.serialize(serializer)
            }
            Amount::Destroyed(of) => TaggedRef::Destroyed(of).serialize(serializer),
            Amount::Exiled(of) => TaggedRef::Exiled(of).serialize(serializer),
            Amount::PhasedOut(of) => TaggedRef::PhasedOut(of).serialize(serializer),
            Amount::Kicks => TaggedRef::Kicks.serialize(serializer),
            Amount::Mutations(of) => TaggedRef::Mutations(of).serialize(serializer),
            Amount::Promised => TaggedRef::Promised.serialize(serializer),
            Amount::ResolvedThisTurn => TaggedRef::ResolvedThisTurn.serialize(serializer),
            Amount::Devoured => TaggedRef::Devoured.serialize(serializer),
            Amount::Bound(name) => TaggedRef::Bound(name).serialize(serializer),
            Amount::CastWith(way) => TaggedRef::CastWith(*way).serialize(serializer),
            Amount::ColorsSpent => TaggedRef::ColorsSpent.serialize(serializer),
            Amount::PermanentsGraved => TaggedRef::PermanentsGraved.serialize(serializer),
            Amount::Tallied { who, what } => {
                TaggedRef::Tallied { who, what: *what }.serialize(serializer)
            }
            Amount::Largest { who, of } => TaggedRef::Largest { who, of }.serialize(serializer),
            Amount::HighestCounters { who, counter } => {
                TaggedRef::HighestCounters { who, counter }.serialize(serializer)
            }
            Amount::StartingLife(who) => TaggedRef::StartingLife(who).serialize(serializer),
            Amount::CastLastTurn(who) => TaggedRef::CastLastTurn(who).serialize(serializer),
            Amount::Party(who) => TaggedRef::Party(who).serialize(serializer),
            Amount::Triggering => TaggedRef::Triggering.serialize(serializer),
            Amount::DieResult(which) => TaggedRef::DieResult(*which).serialize(serializer),
            Amount::Tally { event, who, filter } => TaggedRef::Tally {
                event: *event,
                who,
                filter,
            }
            .serialize(serializer),
            Amount::Remembered { on, slot } => {
                TaggedRef::Remembered { on, slot }.serialize(serializer)
            }
            Amount::CommanderColors(who) => TaggedRef::CommanderColors(who).serialize(serializer),
            Amount::TurnNumber => TaggedRef::TurnNumber.serialize(serializer),
            Amount::TriggeringTargets => TaggedRef::TriggeringTargets.serialize(serializer),
            Amount::When {
                condition,
                then,
                otherwise,
            } => TaggedRef::When {
                condition,
                then,
                otherwise,
            }
            .serialize(serializer),
            Amount::ManaSpent { of } => TaggedRef::ManaSpent { of: *of }.serialize(serializer),
            Amount::ManaColors(who) => TaggedRef::ManaColors(who).serialize(serializer),
            Amount::MostCommon { of, stat } => {
                TaggedRef::MostCommon { of, stat: *stat }.serialize(serializer)
            }
            Amount::Random { min, max } => TaggedRef::Random {
                min: *min,
                max: *max,
            }
            .serialize(serializer),
            Amount::ManaInPool(who) => TaggedRef::ManaInPool(who).serialize(serializer),
            Amount::Doors(of) => TaggedRef::Doors(of).serialize(serializer),
            Amount::Chosen => TaggedRef::Chosen.serialize(serializer),
            Amount::ChosenAs(name) => TaggedRef::ChosenAs(name).serialize(serializer),
            Amount::CountOf { of, filter } => {
                TaggedRef::CountOf { of, filter }.serialize(serializer)
            }
            Amount::Product(parts) => TaggedRef::Product(parts).serialize(serializer),
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
    /// The inner condition is checked as the ability triggers and not again as it resolves (an "if" that is no intervening
    /// "if", Forge's `NoResolvingCheck`).
    AtTrigger(Box<Condition>),
    /// The number of referenced objects that have the selector's characteristics (zone ignored) compares this way.
    CountOf {
        /// The objects counted.
        subject: Ref,
        /// The predicates each must satisfy.
        selector: Selector,
        /// The comparison.
        op: Cmp,
        /// The amount compared with.
        right: Amount,
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
    /// The player the reference names (all of them when it names several) took the first turn of the game.
    StartingPlayer(Ref),
    /// A player the reference names (any of them when it names several) searched their own zone of that kind earlier in
    /// this resolution, whether or not they found a card ("If you search your library this way, shuffle").
    Searched {
        /// Who searched; the controller when absent.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// The zone searched.
        zone: ZoneKind,
    },
    /// The word the resolution's `ChooseValue` of words bound under the name is this one ("on the top or bottom"): false
    /// when nothing was chosen.
    ChosenWord {
        /// The name the word was bound to.
        name: String,
        /// The word asked about.
        word: String,
    },
    /// The effect just before this one in its list was carried out in full ("if you do"): a "may" that was
    /// accepted, a cost that was paid, cards that were moved.
    Performed,
    /// A player the reference names is the monarch (CR 724): any of them when it names several, so `EachPlayer` is "there is a
    /// monarch" and its `Not` "there is no monarch".
    Monarch(Ref),
    /// A player the reference names has the initiative (CR 725, "if you have the initiative").
    Initiative(Ref),
    /// A player the reference names has completed a dungeon this game (CR 309.7, "if you've completed a dungeon").
    CompletedDungeon(Ref),
    /// A player the reference names was the monarch as the turn began: the designation they held before anything in the
    /// turn changed it ("if you were the monarch as the turn began").
    MonarchAtTurnStart(Ref),
    /// The player has the city's blessing (CR 702.131).
    CitysBlessing(Ref),
    /// It is day (CR 726): "if it's day".
    IsDay,
    /// It is night (CR 726): "if it's night".
    IsNight,
    /// It is neither day nor night (CR 726.2): before a card or a permanent with daybound or nightbound has made it day.
    NeitherDayNorNight,
    /// The referenced permanents are controlled by the controller and came under their control since their last turn began (echo, CR 702.30a): read in the upkeep, once that turn has begun.
    NewlyControlled(Ref),
    /// The turn is in this step or main phase: "Activate only during the declare blockers step" (with `YourTurn` for
    /// "during your upkeep").
    Step(Step),
    /// The turn has not reached its declare attackers step: "Activate only ... before attackers are declared".
    BeforeAttackers,
    /// The turn is in its combat phase: "Activate only during combat".
    InCombat,
    /// The turn is in the first combat phase it has had: "if it's the first combat phase of the turn" (CR 500.8).
    FirstCombat,
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
    #[case::plus_one(CounterKind::PlusOnePlusOne, (1, 1))]
    #[case::minus_one(CounterKind::MinusOneMinusOne, (-1, -1))]
    #[case::plus_two_plus_two(CounterKind::Other("+2/+2".into()), (2, 2))]
    #[case::power_only(CounterKind::Other("+1/+0".into()), (1, 0))]
    #[case::toughness_down(CounterKind::Other("+0/-1".into()), (0, -1))]
    #[case::negative_zero(CounterKind::Other("-0/-1".into()), (0, -1))]
    #[case::big(CounterKind::Other("+12/-3".into()), (12, -3))]
    #[case::keyword(CounterKind::Other("flying".into()), (0, 0))]
    #[case::unsigned(CounterKind::Other("2/2".into()), (0, 0))]
    #[case::garbage(CounterKind::Other("+a/+1".into()), (0, 0))]
    #[case::poison(CounterKind::Poison, (0, 0))]
    fn counters_add_power_and_toughness(#[case] kind: CounterKind, #[case] added: (i32, i32)) {
        assert_eq!(kind.power_toughness(), added);
    }

    #[rstest]
    #[case::flashback(CastWay::Flashback, true)]
    #[case::evoke(CastWay::Evoke, true)]
    #[case::overload(CastWay::Overload, true)]
    #[case::madness(CastWay::Madness, true)]
    #[case::suspend(CastWay::Suspend, true)]
    #[case::foretold(CastWay::Foretold, true)]
    #[case::plotted(CastWay::Plotted, true)]
    #[case::escape(CastWay::Escape, true)]
    #[case::dash(CastWay::Dash, true)]
    #[case::blitz(CastWay::Blitz, true)]
    #[case::prowl(CastWay::Prowl, true)]
    #[case::spectacle(CastWay::Spectacle, true)]
    #[case::mayhem(CastWay::Mayhem, true)]
    #[case::disturb(CastWay::Disturb, true)]
    #[case::emerge(CastWay::Emerge, true)]
    #[case::harmonize(CastWay::Harmonize, true)]
    #[case::mutate(CastWay::Mutate, true)]
    #[case::face_down(CastWay::FaceDown, true)]
    #[case::warp(CastWay::Warp, true)]
    #[case::surge(CastWay::Surge, true)]
    #[case::web_slinging(CastWay::WebSlinging, true)]
    #[case::impending(CastWay::Impending, true)]
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
    #[case::half(
        Amount::Half { of: Box::new(Amount::Life(Ref::You)), up: true },
        r#"{"Half":{"of":{"Life":"You"},"up":true}}"#
    )]
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
    #[case::lowest_hand(Amount::LowestHand(Ref::EachPlayer), r#"{"LowestHand":"EachPlayer"}"#)]
    #[case::life_lost_this_way(Amount::LifeLostThisWay, r#""LifeLostThisWay""#)]
    #[case::destroyed_this_way(Amount::Destroyed(Selector(vec![Pred::Type(CardType::Creature)])), r#"{"Destroyed":[{"Type":"Creature"}]}"#)]
    #[case::exiled_this_way(Amount::Exiled(Selector(vec![Pred::Type(CardType::Creature)])), r#"{"Exiled":[{"Type":"Creature"}]}"#)]
    #[case::distinct(Amount::Distinct { of: Selector(vec![Pred::Type(CardType::Creature)]), stat: StatKind::Power }, r#"{"Distinct":{"of":[{"Type":"Creature"}],"stat":"Power"}}"#)]
    #[case::tally(Amount::Tally { event: TallyEvent::TokenCreated, who: Ref::You, filter: Selector(vec![Pred::Type(CardType::Creature)]) }, r#"{"Tally":{"event":"TokenCreated","who":"You","filter":[{"Type":"Creature"}]}}"#)]
    #[case::remembered(Amount::Remembered { on: Ref::This, slot: "n".to_owned() }, r#"{"Remembered":{"on":"This","slot":"n"}}"#)]
    #[case::commander_colors(Amount::CommanderColors(Ref::You), r#"{"CommanderColors":"You"}"#)]
    #[case::turn_number(Amount::TurnNumber, r#""TurnNumber""#)]
    #[case::triggering_targets(Amount::TriggeringTargets, r#""TriggeringTargets""#)]
    #[case::when(
        Amount::When { condition: Box::new(Condition::YourTurn), then: Box::new(Amount::Fixed(3)), otherwise: Box::new(Amount::X) },
        r#"{"When":{"condition":"YourTurn","then":3,"otherwise":"X"}}"#
    )]
    #[case::mana_spent(Amount::ManaSpent { of: Some(ManaType::Red) }, r#"{"ManaSpent":{"of":"Red"}}"#)]
    #[case::all_mana_spent(Amount::ManaSpent { of: None }, r#"{"ManaSpent":{"of":null}}"#)]
    #[case::mana_colors(Amount::ManaColors(Ref::You), r#"{"ManaColors":"You"}"#)]
    #[case::mana_in_pool(Amount::ManaInPool(Ref::You), r#"{"ManaInPool":"You"}"#)]
    #[case::most_common(Amount::MostCommon { of: Selector(vec![Pred::Type(CardType::Creature)]), stat: StatKind::Colors }, r#"{"MostCommon":{"of":[{"Type":"Creature"}],"stat":"Colors"}}"#)]
    #[case::random(Amount::Random { min: 1, max: 6 }, r#"{"Random":{"min":1,"max":6}}"#)]
    #[case::doors(Amount::Doors(Selector(vec![Pred::Type(CardType::Enchantment)])), r#"{"Doors":[{"Type":"Enchantment"}]}"#)]
    #[case::base_power(Amount::Stat { of: Ref::This, stat: StatKind::BasePower }, r#"{"Stat":{"of":"This","stat":"BasePower"}}"#)]
    #[case::base_toughness(Amount::Stat { of: Ref::This, stat: StatKind::BaseToughness }, r#"{"Stat":{"of":"This","stat":"BaseToughness"}}"#)]
    #[case::the_number_a_permanent_chose(Amount::Chosen, r#""Chosen""#)]
    #[case::the_number_a_resolution_chose(Amount::ChosenAs("n".to_owned()), r#"{"ChosenAs":"n"}"#)]
    #[case::c11e_the_other_result(Amount::DieResult(DieResult::Other), r#"{"DieResult":"Other"}"#)]
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
