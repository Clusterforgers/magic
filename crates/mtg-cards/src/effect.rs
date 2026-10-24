//! Effects: the verbs of the DSL. One variant per engine primitive, so a new mechanic is one variant.

use mtg_core::card::{CardType, Subtype};
use mtg_core::mana::ManaType;
use serde::{Deserialize, Serialize};

use crate::card::TokenSpec;
use crate::cost::Cost;
use crate::modify::{CastPermission, Duration, GameRule, Modification};
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, ZoneKind};
use crate::spend::{ManaLasts, SpendOn};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::{Amount, CastWay, Cmp, Condition, CounterKind, DayNight, StatKind};

// serde's `skip_serializing_if` hands out a reference, so these take one.
#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) const fn is_false(value: &bool) -> bool {
    !*value
}

#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) const fn is_true(value: &bool) -> bool {
    *value
}

pub(crate) const fn yes() -> bool {
    true
}

/// A phase an effect adds to the turn (CR 500.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhaseKind {
    /// A beginning phase: untap, upkeep and draw.
    Beginning,
    /// A combat phase.
    Combat,
    /// A main phase, which is a postcombat main phase (CR 505.1a).
    Main,
}

/// Where in a library cards go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Place {
    /// On top.
    Top,
    /// On the bottom.
    Bottom,
    /// Into the library, which is then shuffled.
    Shuffled,
}

/// Who orders several cards put into one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Order {
    /// The player who puts them chooses ("in any order").
    Chosen,
    /// Random order.
    Random,
}

/// Where a moved object goes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Destination {
    /// Its owner's hand.
    Hand,
    /// Its owner's graveyard.
    Graveyard,
    /// The battlefield.
    Battlefield {
        /// Enters tapped.
        #[serde(default, skip_serializing_if = "is_false")]
        tapped: bool,
        /// Enters under this player's control; its owner's when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        under: Option<Ref>,
        /// Enters with its back face up (CR 712.14a); a card that is not a transforming double-faced card stays where it
        /// is.
        #[serde(default, skip_serializing_if = "is_false")]
        transformed: bool,
        /// Counters it enters with, each kind with its number (undying: one +1/+1 counter).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        counters: Vec<(CounterKind, u32)>,
    },
    /// Its owner's library.
    Library {
        /// Top, bottom or shuffled in.
        place: Place,
        /// Who orders several cards.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        order: Option<Order>,
    },
}

/// Mana an effect adds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Produce {
    /// This mana, as printed (`{C}{C}`).
    Fixed(Vec<ManaType>),
    /// This many mana of any one color, chosen when added ("X mana of any one color, where X is the number of Allies you
    /// control"); a count that is not a fixed number is counted when the mana is added.
    AnyColor(Amount),
    /// One mana of a type chosen when added from at least two ("Add {W} or {U}").
    OneOf(Vec<ManaType>),
    /// All the mana of one list, the list chosen when added from at least two ("Add {W}{W}, {W}{U}, or {U}{U}").
    OneOfLists(Vec<Vec<ManaType>>),
    /// This many mana of the color chosen as the source entered the battlefield.
    ChosenColor(u32),
    /// One mana of this type or of the color chosen as the source entered the battlefield ("Add {R} or one mana of the chosen color").
    OrChosenColor(ManaType),
    /// This many mana of any one color in the identity of the controller's commander.
    CommanderIdentity(u32),
    /// This many mana of any one color that an object matching the selector could produce ("any type" adds colorless).
    ProducedBy {
        /// The objects whose mana abilities are looked at.
        among: Selector,
        /// How many mana.
        count: u32,
        /// Colorless counts as a type ("any type that a land you control could produce").
        #[serde(default, skip_serializing_if = "is_false")]
        colorless: bool,
    },
    /// One mana of a type the permanent tapped for mana produced; only in a triggered mana ability (CR 605.1b).
    OfTapped,
    /// The first mana, or the second when the condition holds as the mana is added ("Add {G}. If you control four or more
    /// creatures, add {G}{G} instead."); neither is itself a replacement.
    Instead {
        /// What is added when the condition does not hold.
        base: Box<Produce>,
        /// The condition for the second.
        when: Condition,
        /// What is added in its place when the condition holds.
        instead: Box<Produce>,
    },
    /// This many mana, each of a type chosen when added from `among` or, with none listed, from the five colors ("two mana
    /// in any combination of colors", "X mana in any combination of {R} and/or {G}"). A count of the X announced is the X
    /// of the cost of the ability that adds it.
    AnyCombination {
        /// How many mana.
        count: Amount,
        /// The types to pick from; none means the five colors.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        among: Vec<ManaType>,
    },
}

/// What a created token is.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum TokenRef {
    /// A predefined token by its id in `cards/tokens/` (`treasure`).
    Named(String),
    /// A token described by the effect.
    Custom(Box<TokenSpec>),
    /// An X/X token whose size is an amount counted as the token is created ("an X/X black Horror creature token, where X
    /// is this creature's power"); the description leaves out its power and toughness.
    Sized {
        /// Both its power and its toughness.
        size: Amount,
        /// The rest of the token.
        token: Box<TokenSpec>,
    },
    /// A token that is a copy of the referenced object, with exceptions.
    CopyOf {
        /// The object copied.
        of: Ref,
        /// Changes the copy has ("except it's a Zombie in addition").
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        except: Vec<Modification>,
        /// The copy has no mana cost ("except it's a white Zombie with no mana cost", CR 702.128).
        #[serde(default, skip_serializing_if = "is_false")]
        no_mana_cost: bool,
    },
}

/// One option of a modal spell or ability, with its own targets.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mode {
    /// A short name for prompts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The mode's targets; `Target(i)` in its effects refers to these.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<TargetSpec>,
    /// What the mode does.
    pub effects: Vec<Effect>,
}

/// One row of the table of a die roll: what happens for the totals from `from` to `to`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DieRow {
    /// The lowest total of the row.
    pub from: u32,
    /// The highest total of the row; none when there is no highest ("15+").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<u32>,
    /// What the row does.
    pub effects: Vec<Effect>,
}

/// What the players of a `Vote` vote for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum VoteFor {
    /// Named choices ("votes for time or knowledge"): each name is bound to the players who voted for it, once for each
    /// of their votes, so the size of a name is the number of votes the choice got.
    Named(Vec<String>),
    /// The players or objects a reference names ("votes for a nonland permanent you don't control").
    Of(Ref),
}

/// One instruction of a spell or ability. Optional fields have defaults: `who` is the
/// controller, an `Amount` is one, `source` is the ability's source.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Effect {
    /// The player draws cards.
    Draw {
        /// Who draws.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// The player discards cards of their choice, or at random.
    Discard {
        /// Who discards.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// Cards are chosen at random.
        #[serde(default, skip_serializing_if = "is_false")]
        random: bool,
        /// The name the discarded cards are bound to, as the objects they become in their graveyards (CR 400.7), for the
        /// steps that count them ("the greatest number of cards a player discarded this way").
        #[serde(rename = "as", default, skip_serializing_if = "Option::is_none")]
        bind: Option<String>,
    },
    /// The player mills cards.
    Mill {
        /// Who mills.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// The player scries (CR 701.22).
    Scry {
        /// Who scries.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many cards.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// The controller looks at the top cards of a library and binds them.
    LookAtTop {
        /// Whose library.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many cards.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// The name the cards are bound to.
        #[serde(rename = "as")]
        bind: String,
    },
    /// The objects are revealed to every player.
    Reveal {
        /// What is revealed.
        what: Ref,
    },
    /// The player searches a zone for cards and binds those they find (CR 701.23).
    Search {
        /// Who searches.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// Whose zone: their library unless the zone says otherwise.
        #[serde(
            default = "ZoneKind::library",
            skip_serializing_if = "ZoneKind::is_library"
        )]
        zone: ZoneKind,
        /// What the cards must be.
        filter: Selector,
        /// How many may be found.
        #[serde(default, skip_serializing_if = "Bounds::is_one")]
        count: Bounds,
        /// The cards are revealed.
        #[serde(default, skip_serializing_if = "is_false")]
        reveal: bool,
        /// The name the found cards are bound to.
        #[serde(rename = "as")]
        bind: String,
    },
    /// The player chooses some of a bound set and binds the choice (and optionally the rest).
    Choose {
        /// Who chooses.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// The objects chosen from.
        from: Ref,
        /// How many.
        #[serde(default, skip_serializing_if = "Bounds::is_one")]
        count: Bounds,
        /// The name the chosen objects are bound to.
        #[serde(rename = "as")]
        bind: String,
        /// The name the objects not chosen are bound to.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rest_as: Option<String>,
    },
    /// Cards of a public zone are picked at random, with the game's random source, and the pick is bound; fewer are
    /// picked when the set holds fewer.
    ChooseAtRandom {
        /// The objects picked from.
        from: Ref,
        /// How many are picked.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// The name the picked objects are bound to.
        #[serde(rename = "as")]
        bind: String,
    },
    /// The player shuffles their library.
    Shuffle {
        /// Who shuffles.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// Objects change zones.
    Move {
        /// What moves.
        what: Ref,
        /// Where it goes.
        to: Destination,
    },
    /// Destroys permanents (CR 701.8).
    Destroy {
        /// What is destroyed.
        what: Ref,
        /// They can be regenerated; `false` for "can't be regenerated".
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        regenerate: bool,
    },
    /// Regenerates permanents (CR 701.19): until end of turn, the next time each would be destroyed it is tapped,
    /// loses its damage and leaves combat instead.
    Regenerate {
        /// The permanents given a regeneration shield.
        what: Ref,
    },
    /// Exiles objects.
    Exile {
        /// What is exiled.
        what: Ref,
        /// The name the exiled objects are bound to, as the objects they become in exile (CR 400.7), for the steps that
        /// return them ("exile this Saga, then return it").
        #[serde(rename = "as", default, skip_serializing_if = "Option::is_none")]
        bind: Option<String>,
        /// How long they stay: `WhileSourceOnBattlefield` is "until ~ leaves the battlefield" (CR 610.3), when each returns
        /// to the battlefield under its owner's control as a new object; none exiles them for good.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration: Option<Duration>,
    },
    /// The player puts the top cards of a library onto the battlefield face down as 2/2 creatures they control (manifest,
    /// CR 701.40a); each can be turned face up for its mana cost if it is a creature card.
    Manifest {
        /// Who manifests and controls the permanents.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many cards.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// Whose library the cards come from: their own unless the effect names another player's ("that player's library").
        #[serde(rename = "library", default, skip_serializing_if = "Option::is_none")]
        from: Option<Ref>,
        /// The name the permanents are bound to, as the objects they become on the battlefield.
        #[serde(rename = "as", default, skip_serializing_if = "Option::is_none")]
        bind: Option<String>,
    },
    /// As `Manifest`, and each permanent has ward {2} (cloak, CR 701.58a).
    Cloak {
        /// Who cloaks and controls the permanents.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many cards.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// Whose library the cards come from: their own unless the effect names another player's.
        #[serde(rename = "library", default, skip_serializing_if = "Option::is_none")]
        from: Option<Ref>,
        /// The name the permanents are bound to, as the objects they become on the battlefield.
        #[serde(rename = "as", default, skip_serializing_if = "Option::is_none")]
        bind: Option<String>,
    },
    /// Exiles objects face down, linked to the source of the effect: the permanent hides them (hideaway, CR 702.75a).
    ExileHidden {
        /// What is exiled face down.
        what: Ref,
    },
    /// Exiles the card of the resolving spell encoded on a creature (cipher, CR 702.99a): it lies in exile linked to that
    /// creature, and the link ends when the card leaves exile or the creature leaves the battlefield. Nothing happens
    /// when `what` is no spell card on the stack (a copy of a spell is no card) or `on` names no creature.
    Encode {
        /// The spell card exiled.
        what: Ref,
        /// The creature it is encoded on.
        on: Ref,
    },
    /// The player sacrifices permanents they control that match, of their choice.
    Sacrifice {
        /// Who sacrifices.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// Which permanents may be chosen.
        filter: Selector,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Counters spells or abilities (CR 701.6).
    Counter {
        /// What is countered.
        what: Ref,
    },
    /// Deals damage.
    DealDamage {
        /// The source of the damage; the ability's source when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<Ref>,
        /// Who or what is dealt damage.
        to: Ref,
        /// How much.
        amount: Amount,
    },
    /// The player gains life.
    GainLife {
        /// Who gains.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How much.
        amount: Amount,
    },
    /// The player loses life.
    LoseLife {
        /// Who loses.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How much.
        amount: Amount,
    },
    /// Adds mana to a pool.
    AddMana {
        /// Whose pool.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// The mana.
        mana: Produce,
        /// How many times it is added.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        times: Amount,
        /// What the mana may be spent on, any one of these ("Spend this mana only to cast creature spells"); none means anything.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        only: Vec<SpendOn>,
        /// How long the mana stays in the pool ("Until end of turn, you don't lose this mana as steps and phases end"); left out means until the step ends.
        #[serde(default, skip_serializing_if = "ManaLasts::is_step")]
        lasts: ManaLasts,
    },
    /// Puts counters on an object or player.
    PutCounters {
        /// Where they go.
        on: Ref,
        /// The kind.
        counter: CounterKind,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Removes counters from an object or player; one that has fewer loses what it has.
    RemoveCounters {
        /// Where they come from.
        from: Ref,
        /// The kind.
        counter: CounterKind,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Moves counters from one object onto another (CR 122.5); the first object gives up what it has, up to the count.
    MoveCounters {
        /// The object they leave.
        from: Ref,
        /// The object they go onto.
        to: Ref,
        /// The kind.
        counter: CounterKind,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Multiplies the counters an object or player holds: each one gets `factor` times as many, so "double" puts as many
    /// more as there are (CR 122.1).
    MultiplyCounters {
        /// What holds them.
        on: Ref,
        /// The kind; none means each kind it holds ("double the number of each kind of counter").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        counter: Option<CounterKind>,
        /// What the number is multiplied by; two doubles it.
        #[serde(default = "two", skip_serializing_if = "is_two")]
        factor: u32,
    },
    /// Taps permanents.
    Tap {
        /// What is tapped.
        what: Ref,
    },
    /// Untaps permanents.
    Untap {
        /// What is untapped.
        what: Ref,
    },
    /// Phases permanents out (CR 702.26): they are treated as though they don't exist, with what is attached to them,
    /// until they phase in during their controller's next untap step.
    PhaseOut {
        /// What phases out.
        what: Ref,
    },
    /// Attaches Auras, Equipment or Fortifications to an object (CR 701.3).
    Attach {
        /// The permanents attached; each moves to `to` when it can be attached there.
        what: Ref,
        /// The object they are attached to.
        to: Ref,
    },
    /// Puts a creature that just entered the battlefield into combat, tapped and attacking the player or planeswalker the creature that the cost of the ability returned to its owner's hand was attacking (CR 702.49a, 508.4); it was never declared as an attacker.
    JoinAttack {
        /// The creature that joins the attack.
        what: Ref,
    },
    /// For each attacking creature `of` names (CR 702.116a), creates for every opponent of its controller other than the player it attacks a token copy of it, tapped and attacking that opponent; the tokens are never declared as attackers (CR 508.4).
    MyriadCopies {
        /// The attacking creatures that are copied.
        of: Ref,
        /// Names the tokens for the effects that follow ("exile the tokens at end of combat").
        #[serde(rename = "as", default, skip_serializing_if = "Option::is_none")]
        bind: Option<String>,
    },
    /// Unattaches Auras, Equipment or Fortifications from what they are attached to (CR 701.3d).
    Unattach {
        /// The permanents unattached; one attached to nothing stays as it is.
        what: Ref,
    },
    /// Transforms permanents.
    Transform {
        /// What transforms.
        what: Ref,
    },
    /// Flips flip cards (CR 710): a flip card on the battlefield becomes its bottom half and stays so until it leaves.
    Flip {
        /// What flips.
        what: Ref,
    },
    /// The player surveils (CR 701.25): looks at the top cards of their library and puts any into the graveyard.
    Surveil {
        /// Who surveils.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many cards.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// The player proliferates (CR 701.34): chooses permanents and players with counters and gives each another.
    Proliferate {
        /// Who proliferates.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// Goads creatures (CR 701.15): until the goader's next turn they attack each combat if able and attack a player
    /// other than the goader if able.
    Goad {
        /// The creatures goaded.
        what: Ref,
    },
    /// A creature fights another (CR 701.14): each deals damage equal to its power to the other.
    Fight {
        /// The creature that fights.
        what: Ref,
        /// The creature it fights.
        with: Ref,
    },
    /// The player becomes the monarch (CR 724).
    BecomeMonarch {
        /// Who becomes it.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// It becomes day or night (CR 726.3): the designation changes, and nothing happens when it already is that.
    BecomeDayNight {
        /// What it becomes.
        to: DayNight,
    },
    /// The player amasses (CR 701.47): counters go on an Army they control, a new 0/0 Army token if they have none.
    Amass {
        /// The creature type of the Army token ("Amass Orcs 2").
        subtype: Subtype,
        /// How many +1/+1 counters.
        count: Amount,
    },
    /// The source creature becomes monstrous (CR 701.37): it gets +1/+1 counters.
    Monstrosity {
        /// How many +1/+1 counters.
        count: Amount,
    },
    /// The source Vehicle becomes crewed (CR 702.122) by the creatures tapped to pay the ability, or the source Mount
    /// becomes saddled (CR 702.171) by them when `saddle`: the event cards trigger on; the type change is its own effect.
    Crewed {
        /// The source is a Mount that becomes saddled.
        #[serde(default, skip_serializing_if = "is_false")]
        saddle: bool,
    },
    /// The source Class becomes this level (CR 716.2): its level rises to it; a Class already at that level or above does
    /// nothing.
    BecomeLevel {
        /// The level the Class becomes.
        level: u32,
    },
    /// The source creature adapts (CR 701.46): it gets +1/+1 counters if it has none.
    Adapt {
        /// How many +1/+1 counters.
        count: Amount,
    },
    /// The player bolsters (CR 701.39): puts counters on the creature they control with the least toughness.
    Bolster {
        /// How many +1/+1 counters.
        count: Amount,
    },
    /// The source creature's controller fabricates (CR 702.123): they may put the counters on it, and otherwise create that many Servo tokens; a source that has left the battlefield only makes the tokens.
    Fabricate {
        /// How many +1/+1 counters, or Servo tokens.
        count: Amount,
    },
    /// Creatures explore (CR 701.44): a land on top goes to the hand, any other card gives a counter and may go to the graveyard.
    Explore {
        /// The creatures that explore.
        what: Ref,
    },
    /// Creatures connive (CR 701.50): their controller draws and discards, a nonland discard gives a +1/+1 counter.
    Connive {
        /// The creatures that connive.
        what: Ref,
    },
    /// The player exiles cards from the top of their library, face up one at a time, until one matches the filter
    /// (CR 702.85a, 701.57a); a library that runs out ends it with nothing found.
    ExileUntil {
        /// Whose library.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// What the card that ends the exiling must be.
        filter: Selector,
        /// The name the card found is bound to.
        #[serde(rename = "as")]
        bind: String,
        /// The name every card exiled, the one found included, is bound to.
        exiled_as: String,
    },
    /// The player reveals cards from the top of their library, one at a time, until `count` of them match the filter (CR
    /// 701.20); a library that runs out ends it with the matches found so far. The revealed cards stay in the library
    /// until a later step moves them.
    RevealUntil {
        /// Whose library.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// What each card that counts must be.
        filter: Selector,
        /// How many matching cards end the revealing.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// The name the matching cards are bound to.
        #[serde(rename = "as")]
        bind: String,
        /// The name every card revealed, the matching ones included, is bound to.
        revealed_as: String,
    },
    /// The player exiles cards from the top of their library, face up (CR 406.3) and binds them.
    ExileTop {
        /// Whose library.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many cards.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// The name the cards exiled are bound to, each as the object it becomes in exile.
        #[serde(rename = "as")]
        bind: String,
    },
    /// The player may cast a card during the resolution, without waiting for priority (CR 608.2g); `otherwise` is
    /// done when they do not.
    CastFromZone {
        /// Who casts it.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// The card cast.
        what: Ref,
        /// It is cast without paying its mana cost (CR 118.9).
        #[serde(default, skip_serializing_if = "is_false")]
        free: bool,
        /// The card is played, not cast: a land is put onto the battlefield as a land play when its player could play one now, other cards are cast (CR 305.1, 702.75a).
        #[serde(default, skip_serializing_if = "is_false")]
        play: bool,
        /// The alternative cost it is cast for (CR 118.9): `Madness` pays the madness cost of the card, `Suspend` pays nothing; the spell remembers it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        way: Option<CastWay>,
        /// Done when the card is not cast: the player declines, or nothing lets it be cast.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        otherwise: Vec<Effect>,
    },
    /// The player copies a card in exile (CR 707.12) and binds the copy: a token card in exile that its owner may cast
    /// (`CastFromZone`) and that ceases to exist as soon as it is anywhere but on the stack (CR 704.5e).
    CopyCard {
        /// Who owns the copy.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// The card copied.
        what: Ref,
        /// The name the copy is bound to, as the object it becomes in exile.
        #[serde(rename = "as")]
        bind: String,
    },
    /// The player learns (CR 701.48): they may discard a card, and draw a card if they do; a Lesson from outside the game is not available.
    Learn {
        /// Who learns.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// The player time travels (CR 701.56): each suspended card and permanent with a time counter gains or loses one.
    TimeTravel {
        /// Who time travels.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// The player takes extra turns after this one (CR 500.7): the latest added is taken first.
    ExtraTurn {
        /// Who takes them.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// The untap step of each is skipped ("Skip the untap step of that turn").
        #[serde(default, skip_serializing_if = "is_false")]
        skip_untap: bool,
    },
    /// Phases are added after the current phase (CR 500.8), in this order and before any phase added earlier:
    /// "After this main phase, there is an additional combat phase followed by an additional main phase."
    AddPhase {
        /// The phases added.
        phases: Vec<PhaseKind>,
    },
    /// The player wins the game.
    WinGame {
        /// Who wins.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// Does `effects` once for each player or object `over` names, in the order it lists them, with `bind` naming the one
    /// the pass is for (CR 608.2c: "for each player", "each opponent ... that player"). The list is fixed when the effect
    /// begins; a name bound inside `effects` is bound anew in each pass.
    RepeatEach {
        /// The players or objects to repeat for.
        over: Ref,
        /// The name of the one the pass is for.
        #[serde(rename = "as")]
        bind: String,
        /// What each pass does.
        effects: Vec<Effect>,
    },
    /// The player rolls a die with `sides` sides (CR 706) and adds `plus`; the first row of `results` that holds the total
    /// is done, none when no row does. The roll comes from the game's random stream and is logged as it happened.
    RollDice {
        /// Who rolls.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many faces the die has.
        sides: u32,
        /// Added to the number rolled before the rows are read ("roll a d20 and add the number of cards in your hand").
        #[serde(default = "Amount::zero", skip_serializing_if = "Amount::is_zero")]
        plus: Amount,
        /// What the total does.
        results: Vec<DieRow>,
    },
    /// The player flips a coin (CR 705): `win` is done when they win the flip, `lose` when they lose it. The call carries no
    /// choice, so a flip is won with even odds. The flip comes from the game's random stream and is logged as it happened.
    FlipCoin {
        /// Who flips.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// Done when the flip is won.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        win: Vec<Effect>,
        /// Done when the flip is lost.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        lose: Vec<Effect>,
    },
    /// Each player votes (CR 701.38), the controller first and the others in turn order. A vote on named choices binds
    /// each name to the players who voted for it; a vote on players or objects binds `bind` to those with the most votes,
    /// all of them when tied. A secret vote is shown to the other seats only when every vote is cast.
    Vote {
        /// What is voted for.
        options: VoteFor,
        /// The votes are cast face down and revealed together ("secretly votes for").
        #[serde(default, skip_serializing_if = "is_false")]
        secret: bool,
        /// Names the players or objects with the most votes when `options` is `Of`.
        #[serde(rename = "as", default, skip_serializing_if = "Option::is_none")]
        bind: Option<String>,
    },
    /// Creates tokens.
    CreateToken {
        /// Who controls them.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// What they are.
        token: TokenRef,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// They enter tapped.
        #[serde(default, skip_serializing_if = "is_false")]
        tapped: bool,
        /// They enter attacking.
        #[serde(default, skip_serializing_if = "is_false")]
        attacking: bool,
        /// Names the tokens for the effects that follow ("then attach this to it").
        #[serde(rename = "as", default, skip_serializing_if = "Option::is_none")]
        bind: Option<String>,
    },
    /// Copies spells on the stack (CR 707.10): each copy is put on the stack above the spell, not cast, with the modes, X,
    /// costs paid and targets of the original. Its controller may choose new targets for it when `new_targets` is set.
    CopySpell {
        /// The spells copied.
        what: Ref,
        /// Who controls the copies and chooses their new targets: the controller of the effect unless it names a player.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many copies of each.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// The controller of the copy may choose new targets for it ("You may choose new targets for the copy").
        #[serde(default, skip_serializing_if = "is_false")]
        new_targets: bool,
        /// Changes the copy has ("except that the copy is red").
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        except: Vec<Modification>,
    },
    /// Copies activated and triggered abilities on the stack (CR 707.10): each copy is put on the stack above the ability,
    /// not activated and not triggered, with the choices of the original.
    CopyAbility {
        /// The abilities copied.
        what: Ref,
        /// How many copies of each.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// The controller of the copy may choose new targets for it.
        #[serde(default, skip_serializing_if = "is_false")]
        new_targets: bool,
    },
    /// The permanents become copies of an object for a while ("becomes a copy of target creature until end of turn",
    /// CR 707.2): the copy effect is the first layer, and the permanent is what it was again when it ends.
    BecomeCopy {
        /// The permanents that become copies.
        what: Ref,
        /// The object copied.
        of: Ref,
        /// Changes the copy has.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        except: Vec<Modification>,
        /// How long the copy lasts.
        duration: Duration,
    },
    /// Creates a continuous effect from this resolution; the objects are fixed now (CR 611.2c).
    Continuous {
        /// The objects changed.
        affected: Ref,
        /// The changes.
        mods: Vec<Modification>,
        /// How long they last.
        duration: Duration,
    },
    /// Changes a rule of the game for a while.
    Rule {
        /// The rule.
        rule: GameRule,
        /// How long.
        duration: Duration,
    },
    /// Creates a shield (CR 615): a replacement effect for a while, whose references name what they do now
    /// (CR 611.2c). A damage shield prevents damage ("prevent the next 3 damage that would be dealt to target creature
    /// this turn").
    Shield {
        /// The event it replaces.
        when: ReplaceWhen,
        /// What happens instead.
        by: ReplaceBy,
        /// How long it lasts.
        duration: Duration,
    },
    /// Creates a delayed triggered ability (CR 603.7); bound names are captured as they are now.
    DelayedTrigger {
        /// The event it waits for.
        when: TriggerPattern,
        /// It triggers once and is gone (the usual case).
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        once: bool,
        /// What it does.
        effects: Vec<Effect>,
    },
    /// Does `then` if the condition holds, `otherwise` if not.
    If {
        /// The test.
        condition: Condition,
        /// Done when true.
        then: Vec<Effect>,
        /// Done when false.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        otherwise: Vec<Effect>,
    },
    /// The player may do these effects.
    May {
        /// Who decides.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// What they may do.
        effects: Vec<Effect>,
    },
    /// The player may pay; `paid` follows if they do, `unpaid` if they do not ("unless pays").
    Pay {
        /// Who may pay.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// The cost.
        cost: Cost,
        /// How many times the cost is paid at once ("for each age counter"); nothing is asked when it is 0.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        times: Amount,
        /// Done when the cost is paid.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        paid: Vec<Effect>,
        /// Done when it is not.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        unpaid: Vec<Effect>,
    },
    /// The controller chooses modes when the spell or ability is put on the stack (CR 700.2).
    Modal {
        /// How many modes are chosen.
        #[serde(default, skip_serializing_if = "Bounds::is_one")]
        choose: Bounds,
        /// The same mode may be chosen more than once.
        #[serde(default, skip_serializing_if = "is_false")]
        repeat: bool,
        /// The game picks the modes with its random source instead of the controller ('choose one at random'): the
        /// pick is public. Needs a fixed number of modes and no repeats.
        #[serde(default, skip_serializing_if = "is_false")]
        at_random: bool,
        /// A mode this ability chose earlier in the turn is not offered ('choose one that hasn't been chosen this
        /// turn'); a spell has no earlier choice to keep.
        #[serde(default, skip_serializing_if = "is_false")]
        once_each_turn: bool,
        /// Every mode may be chosen instead when this holds as the ability is announced ("If you control a commander as
        /// you cast this spell, you may choose both instead"); the fewest to choose stays.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        all_if: Option<Condition>,
        /// The modes, in printed order.
        modes: Vec<Mode>,
    },
}

/// The name the card found by exiling until a card matches is bound to.
pub const FOUND: &str = "found";

/// The name every card exiled that way is bound to, the card found included.
pub const EXILED: &str = "exiled";

/// The name every card revealed until a card matches is bound to, the cards found included.
pub const REVEALED: &str = "revealed";

/// The names the effects bind, those inside the lists nested in them too: the fields written `as`, `rest_as`, `exiled_as`
/// and `revealed_as`, sorted.
pub fn bound_names(effects: &[Effect]) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(value) = serde_json::to_value(effects) {
        collect_names(&value, &mut names);
    }
    names.sort();
    names.dedup();
    names
}

fn collect_names(value: &serde_json::Value, names: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, inner) in fields {
                match inner.as_str() {
                    Some(name)
                        if matches!(
                            key.as_str(),
                            "as" | "rest_as" | "exiled_as" | "revealed_as"
                        ) =>
                    {
                        names.push(name.to_owned());
                    }
                    _ => collect_names(inner, names),
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_names(item, names);
            }
        }
        _ => {}
    }
}

/// The factor of `MultiplyCounters` when none is written: counters are doubled.
const fn two() -> u32 {
    2
}

/// Whether a factor is the default one, so serialization can leave it out (serde hands over a reference).
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_two(factor: &u32) -> bool {
    *factor == 2
}

/// The steps that exile cards from the top of the library until one matches `filter`, let the player cast it free (and do
/// `otherwise` when they do not) and put every card exiled that is still in exile on the bottom in a random order.
fn exile_cast_bottom(filter: Selector, otherwise: Vec<Effect>) -> Vec<Effect> {
    vec![
        Effect::ExileUntil {
            who: Ref::You,
            filter,
            bind: FOUND.to_owned(),
            exiled_as: EXILED.to_owned(),
        },
        Effect::CastFromZone {
            who: Ref::You,
            what: Ref::Bound(FOUND.to_owned()),
            free: true,
            play: false,
            way: None,
            otherwise,
        },
        Effect::Move {
            what: Ref::Bound(EXILED.to_owned()),
            to: Destination::Library {
                place: Place::Bottom,
                order: Some(Order::Random),
            },
        },
    ]
}

/// The steps of discover N (CR 701.57a): the first nonland card of mana value `count` or less is cast free or put into the
/// hand, and the cards exiled with it go to the bottom of the library.
pub fn discover(count: Amount) -> Vec<Effect> {
    exile_cast_bottom(
        Selector(vec![
            Pred::Not(Box::new(Pred::Type(CardType::Land))),
            Pred::Compare {
                stat: StatKind::ManaValue,
                op: Cmp::Le,
                to: count,
            },
        ]),
        vec![Effect::Move {
            what: Ref::Bound(FOUND.to_owned()),
            to: Destination::Hand,
        }],
    )
}

/// The steps of cascade (CR 702.85a): the first nonland card of lesser mana value than the source, the spell that cascades,
/// may be cast free, and the cards exiled with it go to the bottom of the library.
pub fn cascade() -> Vec<Effect> {
    exile_cast_bottom(
        Selector(vec![
            Pred::Not(Box::new(Pred::Type(CardType::Land))),
            Pred::Compare {
                stat: StatKind::ManaValue,
                op: Cmp::Lt,
                to: Amount::Stat {
                    of: Ref::This,
                    stat: StatKind::ManaValue,
                },
            },
        ]),
        Vec::new(),
    )
}

/// The name the cards an impulse draw exiles are bound to.
pub const IMPULSE: &str = "impulse";

/// The steps of an impulse draw: the controller exiles the top `count` cards of their library face up and may play them
/// (cast them, or play a land among them) until `duration` ends (CR 601.3, 305.1). A card that leaves exile is a new object
/// the permission no longer covers.
pub fn impulse(count: Amount, duration: Duration) -> Vec<Effect> {
    vec![
        Effect::ExileTop {
            who: Ref::You,
            count,
            bind: IMPULSE.to_owned(),
        },
        Effect::Rule {
            rule: GameRule::MayCast(CastPermission {
                who: Ref::You,
                what: Selector::default(),
                from: Some(ZoneKind::Exile),
                free: false,
                flash: false,
                only: Some(Ref::Bound(IMPULSE.to_owned())),
                play: true,
            }),
            duration,
        },
    ]
}

/// The name the cards a hideaway permanent looks at are bound to.
pub const LOOKED: &str = "looked";

/// The name the card a hideaway permanent exiles face down is bound to.
pub const HIDDEN: &str = "hidden";

/// The name the cards a hideaway permanent puts on the bottom are bound to.
pub const UNHIDDEN: &str = "unhidden";

/// The steps of hideaway N (CR 702.75a): the controller looks at the top `count` cards of their library, exiles one face
/// down, linked to the source, and puts the rest on the bottom in a random order.
pub fn hideaway(count: Amount) -> Vec<Effect> {
    vec![
        Effect::LookAtTop {
            who: Ref::You,
            count,
            bind: LOOKED.to_owned(),
        },
        Effect::Choose {
            who: Ref::You,
            from: Ref::Bound(LOOKED.to_owned()),
            count: Bounds::default(),
            bind: HIDDEN.to_owned(),
            rest_as: Some(UNHIDDEN.to_owned()),
        },
        Effect::ExileHidden {
            what: Ref::Bound(HIDDEN.to_owned()),
        },
        Effect::Move {
            what: Ref::Bound(UNHIDDEN.to_owned()),
            to: Destination::Library {
                place: Place::Bottom,
                order: Some(Order::Random),
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    //! Goal: effects read and write the compact forms card files use, with defaults left out, and refuse what is not a field.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::draw_one(Effect::Draw { who: Ref::You, count: Amount::one() }, r#"{"Draw":{}}"#)]
    #[case::opponent_draws(
        Effect::Draw { who: Ref::Target(0), count: Amount::Fixed(2) },
        r#"{"Draw":{"who":{"Target":0},"count":2}}"#
    )]
    #[case::bolt(
        Effect::DealDamage { source: None, to: Ref::Target(0), amount: Amount::Fixed(3) },
        r#"{"DealDamage":{"to":{"Target":0},"amount":3}}"#
    )]
    #[case::wrath(
        Effect::Destroy { what: Ref::Each(Selector(vec![Pred::Type(CardType::Creature)])), regenerate: false },
        r#"{"Destroy":{"what":{"Each":[{"Type":"Creature"}]},"regenerate":false}}"#
    )]
    #[case::destroy_default(
        Effect::Destroy { what: Ref::Target(0), regenerate: true },
        r#"{"Destroy":{"what":{"Target":0}}}"#
    )]
    #[case::move_to_battlefield_tapped(
        Effect::Move { what: Ref::Bound("land".into()), to: Destination::Battlefield { tapped: true, under: None, transformed: false, counters: vec![] } },
        r#"{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}}"#
    )]
    #[case::search(
        Effect::Search { who: Ref::You, zone: ZoneKind::Library, filter: Selector(vec![Pred::Supertype(mtg_core::card::Supertype::Basic)]), count: Bounds::default(), reveal: false, bind: "land".into() },
        r#"{"Search":{"filter":[{"Supertype":"Basic"}],"as":"land"}}"#
    )]
    #[case::pay(
        Effect::Pay { who: Ref::Triggering, cost: Cost::mana("{1}".parse().unwrap()), times: Amount::one(), paid: vec![], unpaid: vec![Effect::Draw { who: Ref::You, count: Amount::one() }] },
        r#"{"Pay":{"who":"Triggering","cost":"{1}","unpaid":[{"Draw":{}}]}}"#
    )]
    #[case::put_a_poison_counter_on_a_player(
        Effect::PutCounters { on: Ref::Target(0), counter: CounterKind::Poison, count: Amount::one() },
        r#"{"PutCounters":{"on":{"Target":0},"counter":"poison"}}"#
    )]
    #[case::remove_all_the_counters_of_a_kind(
        Effect::RemoveCounters { from: Ref::This, counter: CounterKind::Other("energy".into()), count: Amount::Counters { on: Ref::This, counter: CounterKind::Other("energy".into()) } },
        r#"{"RemoveCounters":{"from":"This","counter":{"Other":"energy"},"count":{"Counters":{"on":"This","counter":{"Other":"energy"}}}}}"#
    )]
    #[case::double_the_counters_of_a_kind(
        Effect::MultiplyCounters { on: Ref::This, counter: Some(CounterKind::PlusOnePlusOne), factor: 2 },
        r#"{"MultiplyCounters":{"on":"This","counter":"+1/+1"}}"#
    )]
    #[case::triple_each_kind(
        Effect::MultiplyCounters { on: Ref::Target(0), counter: None, factor: 3 },
        r#"{"MultiplyCounters":{"on":{"Target":0},"factor":3}}"#
    )]
    #[case::repeat_for_each_opponent(
        Effect::RepeatEach { over: Ref::Opponents, bind: "foe".to_owned(), effects: vec![Effect::Draw { who: Ref::Bound("foe".to_owned()), count: Amount::one() }] },
        r#"{"RepeatEach":{"over":"Opponents","as":"foe","effects":[{"Draw":{"who":{"Bound":"foe"}}}]}}"#
    )]
    #[case::roll_a_die_with_a_table(
        Effect::RollDice { who: Ref::You, sides: 20, plus: Amount::Fixed(2), results: vec![DieRow { from: 1, to: Some(9), effects: vec![Effect::Shuffle { who: Ref::You }] }, DieRow { from: 10, to: None, effects: vec![Effect::Shuffle { who: Ref::You }] }] },
        r#"{"RollDice":{"sides":20,"plus":2,"results":[{"from":1,"to":9,"effects":[{"Shuffle":{}}]},{"from":10,"effects":[{"Shuffle":{}}]}]}}"#
    )]
    #[case::flip_a_coin(
        Effect::FlipCoin { who: Ref::You, win: vec![Effect::Draw { who: Ref::You, count: Amount::one() }], lose: Vec::new() },
        r#"{"FlipCoin":{"win":[{"Draw":{}}]}}"#
    )]
    #[case::vote_on_two_names(
        Effect::Vote { options: VoteFor::Named(vec!["time".to_owned(), "knowledge".to_owned()]), secret: true, bind: None },
        r#"{"Vote":{"options":{"Named":["time","knowledge"]},"secret":true}}"#
    )]
    #[case::vote_on_objects(
        Effect::Vote { options: VoteFor::Of(Ref::Each(Selector(vec![Pred::In(ZoneKind::Battlefield)]))), secret: false, bind: Some("top".to_owned()) },
        r#"{"Vote":{"options":{"Of":{"Each":[{"In":"Battlefield"}]}},"as":"top"}}"#
    )]
    #[case::attach_the_source_to_a_target(
        Effect::Attach { what: Ref::This, to: Ref::Target(0) },
        r#"{"Attach":{"what":"This","to":{"Target":0}}}"#
    )]
    #[case::unattach_the_source(Effect::Unattach { what: Ref::This }, r#"{"Unattach":{"what":"This"}}"#)]
    #[case::move_two_counters(
        Effect::MoveCounters { from: Ref::Target(0), to: Ref::Target(1), counter: CounterKind::PlusOnePlusOne, count: Amount::Fixed(2) },
        r#"{"MoveCounters":{"from":{"Target":0},"to":{"Target":1},"counter":"+1/+1","count":2}}"#
    )]
    fn effects_have_stable_json(#[case] value: Effect, #[case] json: &str) {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(serde_json::from_str::<Effect>(json).unwrap(), value);
    }

    #[rstest]
    #[case::nothing_bound(r#"[{"Draw":{}}]"#, &[])]
    #[case::one_name(r#"[{"LookAtTop":{"as":"looked"}}]"#, &["looked"])]
    #[case::the_names_of_a_choice(r#"[{"Choose":{"from":"Opponents","as":"one","rest_as":"others"}}]"#, &["one", "others"])]
    #[case::the_names_of_a_reveal(r#"[{"RevealUntil":{"filter":[],"as":"found","revealed_as":"seen"}}]"#, &["found", "seen"])]
    #[case::inside_nested_lists(
        r#"[{"If":{"condition":"YourTurn","then":[{"ExileTop":{"as":"top"}}],"otherwise":[{"RepeatEach":{"over":"Opponents","as":"foe","effects":[{"ExileTop":{"as":"inner"}}]}}]}}]"#,
        &["foe", "inner", "top"]
    )]
    fn the_names_effects_bind_are_found_in_nested_lists(
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        let effects: Vec<Effect> = serde_json::from_str(json).unwrap();
        assert_eq!(bound_names(&effects), expected);
    }

    #[rstest]
    #[case::unknown_field(r#"{"Draw":{"cards":2}}"#, "unknown field `cards`")]
    #[case::unknown_variant(r#"{"Teleport":{}}"#, "unknown variant `Teleport`")]
    #[case::missing_field(r#"{"DealDamage":{"amount":3}}"#, "missing field `to`")]
    #[case::bad_bind(r#"{"LookAtTop":{}}"#, "missing field `as`")]
    fn bad_effects_say_why(#[case] json: &str, #[case] expected: &str) {
        let error = serde_json::from_str::<Effect>(json)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }
}
