//! Patterns over game events: what triggers an ability and what a replacement effect replaces.

use mtg_core::color::Color;
use serde::{Deserialize, Serialize};

use crate::did::Action;
use crate::effect::{Effect, Place, is_false};
use crate::modify::Modification;
use crate::refs::{Ref, Selector, ZoneKind};
use crate::value::{Amount, Condition, CounterKind, DayNight};

/// A step or main phase of a turn, as triggers name it (the same thirteen as the engine's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Step {
    /// Untap step.
    Untap,
    /// Upkeep step.
    Upkeep,
    /// Draw step.
    Draw,
    /// Precombat main phase.
    Main1,
    /// Beginning of combat step.
    BeginCombat,
    /// Declare attackers step.
    DeclareAttackers,
    /// Declare blockers step.
    DeclareBlockers,
    /// First-strike combat damage step.
    FirstStrikeDamage,
    /// Combat damage step.
    CombatDamage,
    /// End of combat step.
    EndCombat,
    /// Postcombat main phase.
    Main2,
    /// End step.
    End,
    /// Cleanup step.
    Cleanup,
}

// serde's `default` and `skip_serializing_if` take a function and a reference.
const fn one_attacker() -> u32 {
    1
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_one_attacker(count: &u32) -> bool {
    *count == 1
}

/// The game event a triggered ability waits for (CR 603.2). Each variant is backed by one or more
/// engine events (`docs/cards-dsl.md`, Triggers, replacement and layers).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum TriggerPattern {
    /// An object matching the selector moves between zones: enters (`to: Battlefield`), dies
    /// (`from: Battlefield, to: Graveyard`), leaves, is put into a graveyard, and so on.
    ZoneChange {
        /// The object that moves.
        what: Selector,
        /// The zone it leaves, if the pattern names one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<ZoneKind>,
        /// The zone it enters, if the pattern names one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<ZoneKind>,
    },
    /// An object matching the selector moves to a zone from any zone but one ("is put into a graveyard from anywhere
    /// other than the battlefield"); a token that is created moves from no zone and so never matches.
    ZoneChangeExcept {
        /// The object that moves.
        what: Selector,
        /// The zone it does not leave.
        except: ZoneKind,
        /// The zone it enters, if the pattern names one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<ZoneKind>,
    },
    /// A spell matching the selector resolved and its card was put into a graveyard ("when this spell card is put into a
    /// graveyard after resolving", haunt CR 702.55a); a countered spell is not.
    ResolvedToGraveyard {
        /// The spell.
        what: Selector,
    },
    /// A creature matching the selector attacks.
    Attacks {
        /// The attacker.
        what: Selector,
    },
    /// A creature matching the selector attacks alone: it is the only creature declared as an attacker; `Triggering` is
    /// the attacker.
    AttacksAlone {
        /// The attacker.
        what: Selector,
    },
    /// A creature matching the selector attacks a player ("whenever a creature attacks enchanted player"); `Triggering`
    /// is the attacker.
    AttacksPlayer {
        /// The attacker.
        what: Selector,
        /// The player attacked.
        at: Ref,
    },
    /// A creature matching the selector attacks a player or a planeswalker that player controls ("whenever a creature
    /// attacks you or a planeswalker you control"); `Triggering` is the attacker.
    AttacksPlayerOrPlaneswalker {
        /// The attacker.
        what: Selector,
        /// The player attacked, or the controller of the planeswalker attacked.
        at: Ref,
    },
    /// A player is attacked ("whenever enchanted player is attacked"): creatures are declared as attackers of that
    /// player (an attack on only their planeswalkers is not one). It triggers once for the declaration, however many
    /// creatures attack; `Triggering` is the attacked player.
    PlayerAttacked {
        /// The player attacked.
        who: Ref,
    },
    /// A player attacks a player with one or more creatures ("whenever a player attacks enchanted player with one or
    /// more creatures"). It triggers once for each attacking player; `Triggering` is the attacking player.
    PlayerAttacks {
        /// The attacking player.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        by: Ref,
        /// The player attacked.
        at: Ref,
    },
    /// An object matching the selector becomes the target of a spell or ability.
    BecomesTarget {
        /// The object targeted.
        what: Selector,
        /// The spell or ability that targets it; anything when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        by: Selector,
    },
    /// A player the reference names becomes the target of a spell or ability matching `by` ("whenever you become the
    /// target of a spell or ability an opponent controls"). `Triggering` is the spell or ability, and as a player its
    /// controller.
    PlayerBecomesTarget {
        /// The player targeted.
        who: Ref,
        /// The spell or ability that targets them; anything when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        by: Selector,
    },
    /// A spell matching the selector is cast.
    SpellCast {
        /// The spell; any spell when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// Who casts it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        by: Ref,
    },
    /// A player copies a spell matching the selector (CR 707.10): the copy is put on the stack under their control.
    SpellCopied {
        /// The spell copied; any spell when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// Who copies it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        by: Ref,
    },
    /// A permanent matching the selector is sacrificed.
    Sacrificed {
        /// The permanent.
        what: Selector,
        /// Who sacrifices it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        by: Ref,
    },
    /// A card matching the selector is milled (CR 701.17): put from its owner's library into their graveyard by a mill,
    /// never by a surveil or another move. The card is the one subject, so a batch of them triggers once, unless
    /// `per_player` makes the owner a subject too.
    Milled {
        /// The card milled.
        what: Selector,
        /// Who mills it (its owner).
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        by: Ref,
        /// The player who milled is a subject, so a batch triggers once for each player ("whenever a player mills one
        /// or more creature cards").
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        per_player: bool,
    },
    /// A source deals damage to a player or a permanent.
    DealsDamage {
        /// The source of the damage.
        source: Selector,
        /// The recipient; any player or permanent when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<Ref>,
        /// Only combat damage (`true`) or only other damage (`false`); both when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        combat: Option<bool>,
    },
    /// A permanent the selector picks is dealt excess damage (CR 120.10): more damage at once than is lethal to it, its
    /// toughness less the damage marked for a creature, its loyalty for a planeswalker; `Amount::Triggering` is the
    /// excess, and the permanent is the one triggering.
    DealtExcess {
        /// The permanents.
        what: Selector,
        /// Only combat damage (`true`) or only other damage (`false`); both when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        combat: Option<bool>,
    },
    /// Damage that would be dealt to a player is prevented ("whenever damage that would be dealt to you is prevented");
    /// the amount of the event is the damage prevented.
    DamagePrevented {
        /// The player it would have been dealt to.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A player gains life.
    LifeGained {
        /// Who gains it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A player loses life.
    LifeLost {
        /// Who loses it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// The controller of the ability revealed the first card they drew this turn through the `RevealFirstDraw` rule
    /// of the same permanent ("Whenever you reveal a creature card this way"); `Triggering` is the card.
    FirstDrawRevealed {
        /// The card revealed; any card when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
    },
    /// A player draws a card.
    Drew {
        /// Who draws.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
        /// The card drawn ("when you draw this card"); any card when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// Only the first card the player draws in each of their draw steps (`true`), or every card but that one
        /// (`false`, "except the first one they draw in each of their draw steps"); any card when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        first_in_draw_step: Option<bool>,
    },
    /// A player discards a card.
    Discarded {
        /// The card; any card when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// Who discards it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        by: Ref,
    },
    /// A permanent matching the selector becomes tapped.
    Tapped {
        /// The permanent.
        what: Selector,
    },
    /// A permanent becomes tapped, not as it attacks.
    TappedNotAttacking {
        /// The permanent.
        what: Selector,
    },
    /// A permanent matching the selector becomes untapped.
    Untapped {
        /// The permanent.
        what: Selector,
        /// Only in the untap step of the turn of the ability's controller ("becomes untapped during your untap step").
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        untap_step: bool,
    },
    /// A permanent matching the selector phases out (CR 702.26); judged as it was before it phased out.
    PhasedOut {
        /// The permanent.
        what: Selector,
    },
    /// A permanent matching the selector phases in (CR 702.26).
    PhasedIn {
        /// The permanent.
        what: Selector,
    },
    /// A permanent matching the selector is tapped for mana.
    TappedForMana {
        /// The permanent.
        what: Selector,
        /// Who taps it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        by: Ref,
        /// Only a tap that makes mana of this type ("Whenever you tap a permanent for {C}"); any tap for mana when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        produced: Option<mtg_core::mana::ManaType>,
        /// Only a tap that makes mana of the color the source chose as it entered ("tapped for mana of the chosen
        /// color"); a source that chose no color makes no tap match.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        chosen_color: bool,
    },
    /// A creature matching the selector blocks.
    Blocks {
        /// The blocker.
        what: Selector,
        /// Only when it blocks one or more attackers matching this (empty: any). With it the trigger happens once for
        /// each such blocker, against the first such attacker it blocks.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        blocking: Selector,
    },
    /// A creature matching the selector becomes blocked.
    BecomesBlocked {
        /// The attacker.
        what: Selector,
        /// Only when blocked by a creature matching this (empty: any blocker). With it the trigger happens once for each such blocker and `Ref::Dealer` names that blocker.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        by: Selector,
        /// With `by`: the trigger happens once when one or more of its blockers match ("becomes blocked by one or more
        /// Orcs"), and names no blocker.
        #[serde(default, skip_serializing_if = "crate::effect::is_false")]
        once: bool,
    },
    /// A creature matching the selector attacks and isn't blocked: it is unblocked once its defending player has
    /// declared blockers (CR 509.1h).
    Unblocked {
        /// The attacker.
        what: Selector,
    },
    /// At least this many creatures matching the selector are declared as attackers together; it triggers once.
    GroupAttacks {
        /// The attackers.
        what: Selector,
        /// The fewest that make it trigger.
        #[serde(default = "one_attacker", skip_serializing_if = "is_one_attacker")]
        at_least: u32,
    },
    /// At least this many creatures matching the selector are declared as attackers of one player together ("whenever one
    /// or more Dragons you control attack an opponent"): it triggers once for each player attacked, with the attackers of
    /// that player; an attack on a planeswalker is no attack on its controller unless `walkers`.
    GroupAttacksAt {
        /// The attackers.
        what: Selector,
        /// The player attacked.
        at: Ref,
        /// The fewest that make it trigger.
        #[serde(default = "one_attacker", skip_serializing_if = "is_one_attacker")]
        at_least: u32,
        /// The attackers of the planeswalkers the player controls count with theirs ("attack one of your opponents or a
        /// planeswalker they control").
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        walkers: bool,
    },
    /// Counters are put on a permanent.
    CountersPut {
        /// What receives them; any permanent when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// Only counters of this kind; any kind when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        counter: Option<CounterKind>,
        /// In a `Batch`, the counters put on each permanent are an event of their own ("one or more +1/+1 counters are
        /// put on a creature you control", CR 603.2c); without it the batch is one event ("on one or more creatures").
        #[serde(default, skip_serializing_if = "is_false")]
        each: bool,
    },
    /// Counters are put on a player ("whenever you get one or more {E}"); `Triggering` is that player.
    PlayerCountersPut {
        /// Who gets them.
        who: Ref,
        /// Only counters of this kind; any kind when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        counter: Option<CounterKind>,
    },
    /// Counters are removed from a permanent.
    CountersRemoved {
        /// What loses them; any permanent when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// Only counters of this kind; any kind when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        counter: Option<CounterKind>,
    },
    /// A chapter ability of a Saga (CR 714.2): the lore counters on `This` reach or pass the number.
    Chapter {
        /// The chapter number.
        number: u32,
    },
    /// The final chapter ability of a Saga matching the selector triggers or, with `resolves`, resolves (CR 714.2c):
    /// the chapter ability with the greatest number the Saga has. `Triggering` is the Saga.
    FinalChapter {
        /// The Saga.
        what: Selector,
        /// It resolves; it triggers when false.
        #[serde(default, skip_serializing_if = "is_false")]
        resolves: bool,
    },
    /// The counters of a kind on `This` reach or pass a number as counters of that kind are put on it ("When the fourth
    /// plan counter is put on this enchantment"); `Triggering` is the permanent.
    NthCounterPut {
        /// The kind of counter.
        counter: CounterKind,
        /// The number the counters reach, from 1.
        number: u32,
    },
    /// An Aura, Equipment or Fortification becomes attached to a permanent.
    Attached {
        /// The attachment; any when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// The permanent it becomes attached to; any when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        to: Selector,
    },
    /// An attachment becomes unattached from a permanent (unattached, moved, or leaving); the permanent it left triggers.
    Unattached {
        /// The attachment; any when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// The permanent it was attached to; any when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        from: Selector,
    },
    /// A card is cycled (CR 702.29).
    Cycled {
        /// The card; any card when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// Who cycles it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        by: Ref,
    },
    /// A player ventures into the dungeon ("whenever you venture into the dungeon"), or, with `room`, enters that room
    /// of the dungeon that is the source (a room ability, CR 309.4c); `Triggering` is that player.
    Ventured {
        /// Who ventures.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
        /// The room of the source dungeon, as an index into its rooms.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        room: Option<u8>,
    },
    /// A player completes a dungeon ("whenever you complete a dungeon", CR 309.7); `Triggering` is that player.
    CompletedDungeon {
        /// Who completes it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// The visit ability of an Attraction (CR 702.159a, "Visit — "): its controller rolls to visit their Attractions and
    /// the result is a number lit up on the source; `Triggering` is that player.
    Visit {},
    /// A player rolls to visit their Attractions ("whenever you roll to visit your Attractions", CR 701.52);
    /// `Triggering` is that player and `Amount::Triggering` the number rolled.
    RolledToVisit {
        /// Who rolls.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A player visits an Attraction ("whenever you visit an Attraction"): once for each Attraction a roll to visit
    /// visits; `Triggering` is that player.
    VisitedAttraction {
        /// Who visits it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// The prize ability of an Attraction ("Prize — "): its controller claims the prize of the source; `Triggering` is
    /// that player.
    Prize {},
    /// A player claims the prize of an Attraction ("whenever you claim the prize of an Attraction"); `Triggering` is
    /// that player.
    ClaimedPrize {
        /// Who claims it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A player opens an Attraction ("whenever you open an Attraction", CR 701.51c); `Triggering` is that player.
    OpenedAttraction {
        /// Who opens it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A player unlocks a door of a Room (CR 709.5): "when you unlock this door" (`door`, the face of the source, set as the
    /// card is compiled) or "whenever you fully unlock a Room" (`fully`, its last locked door); `Triggering` is that player.
    DoorUnlocked {
        /// Who unlocks it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
        /// The door of the source Room, as an index into its faces.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        door: Option<u8>,
        /// Only the unlock of the last locked door counts.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        fully: bool,
    },
    /// A player takes the initiative (CR 725); `Triggering` is that player.
    TakesInitiative {
        /// Who takes it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A player becomes the monarch ("whenever you become the monarch"); `Triggering` is that player.
    BecomesMonarch {
        /// Who becomes it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// Day becomes night or night becomes day (CR 726.3): not the first time it becomes day from neither.
    DayNightChanges {
        /// Only this change ("whenever night becomes day"); either change when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<DayNight>,
    },
    /// The Ring tempts a player ("whenever the Ring tempts you"); `Triggering` is that player. With `bearer` it needs a
    /// creature chosen as the Ring-bearer that matches ("whenever you choose a creature other than ~ as your Ring-bearer").
    RingTempts {
        /// Who is tempted.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
        /// The creature chosen as Ring-bearer must match this; any tempting when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        bearer: Selector,
    },
    /// A player proliferates ("whenever you proliferate"); `Triggering` is that player.
    Proliferated {
        /// Who proliferates.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A player rolls one or more dice ("whenever you roll one or more dice"), once for each roll; with `highest`, once
    /// for each die that rolls its highest natural result ("whenever you roll a die's highest natural result", CR 706.2);
    /// with `results`, once for each die whose result is one of them ("whenever you roll a 1 or 2"), and `Amount::Triggering`
    /// is that result. `Triggering` is that player.
    DiceRolled {
        /// Who rolls.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
        /// Only a die that rolls its number of faces.
        #[serde(default, skip_serializing_if = "crate::effect::is_false")]
        highest: bool,
        /// Only a die whose result is one of these.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        results: Vec<u32>,
        /// The results are worded as natural results ("a natural 20"); no effect modifies a roll, so it only words them.
        #[serde(default, skip_serializing_if = "crate::effect::is_false")]
        natural: bool,
    },
    /// A player wins a coin flip ("whenever you win a coin flip", CR 705.2); `Triggering` is that player.
    CoinFlipWon {
        /// Who wins the flip.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A player wins a clash ("whenever you win a clash", CR 701.23); `Triggering` is that player.
    W15cClashWon {
        /// Who wins the clash.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A player amasses ("whenever you amass"); `Triggering` is that player.
    Amassed {
        /// Who amasses.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A creature matching the selector explores ("whenever a creature you control explores"); `Triggering` is its
    /// controller.
    Explored {
        /// The creature that explores.
        what: Selector,
    },
    /// A creature matching the selector connives ("whenever a creature you control connives"); `Triggering` is its
    /// controller.
    Connived {
        /// The creature that connives.
        what: Selector,
    },
    /// A creature matching the selector becomes monstrous ("when this creature becomes monstrous"); `Triggering` is its
    /// controller.
    BecomesMonstrous {
        /// The creature that becomes monstrous.
        what: Selector,
    },
    /// A creature matching the selector becomes renowned ("when this creature becomes renowned", CR 702.112);
    /// `Triggering` is its controller.
    BecomesRenowned {
        /// The creature that becomes renowned.
        what: Selector,
    },
    /// A creature matching the selector exploits a creature ("when this creature exploits a creature", CR 702.110),
    /// judged as the battlefield was before the sacrifice; `Triggering` is its controller.
    Exploits {
        /// The creature that exploits.
        what: Selector,
    },
    /// A Vehicle matching the selector becomes crewed, a crew ability of it resolved ("whenever this Vehicle becomes
    /// crewed", CR 702.122); `Triggering` is its controller.
    BecomesCrewed {
        /// The Vehicle that becomes crewed.
        what: Selector,
    },
    /// A Mount matching the selector becomes saddled, a saddle ability of it resolved ("whenever this creature becomes
    /// saddled", CR 702.171); `Triggering` is its controller.
    BecomesSaddled {
        /// The Mount that becomes saddled.
        what: Selector,
    },
    /// A permanent matching the selector is exerted (CR 701.43): "When you do" after "You may exert this creature as it
    /// attacks", and "Whenever you exert a creature"; `Triggering` is the permanent exerted.
    Exerted {
        /// The permanent exerted.
        what: Selector,
    },
    /// A permanent matching the selector is turned face up ("when this creature is turned face up", CR 702.37e); `Triggering` is
    /// its controller.
    TurnedFaceUp {
        /// The permanent that is turned face up.
        what: Selector,
    },
    /// A creature matching the selector mutates, a mutating creature spell merges with it ("whenever this creature
    /// mutates", CR 702.140d); `Triggering` is its controller.
    Mutates {
        /// The creature that mutates.
        what: Selector,
    },
    /// A Class matching the selector becomes this level ("When this Class becomes level 2", CR 716.2); `Triggering` is
    /// its controller.
    BecomesLevel {
        /// The Class that gains the level.
        what: Selector,
        /// The level it becomes.
        level: u32,
    },
    /// A creature matching the selector adapts ("whenever a creature you control adapts"); `Triggering` is its
    /// controller.
    Adapted {
        /// The creature that adapts.
        what: Selector,
    },
    /// A player bolsters ("whenever you bolster"); `Triggering` is that player.
    Bolstered {
        /// Who bolsters.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A player expends N ("whenever you expend 4", CR 701.59): they spend their Nth total mana to cast spells during a
    /// turn; `Triggering` is that player.
    Expend {
        /// Who expends.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
        /// Which total of mana, from one.
        n: u32,
    },
    /// A player commits a crime ("whenever you commit a crime"): they cast a spell, activate an ability or put a triggered
    /// ability on the stack that targets an opponent, an object an opponent controls or a card in an opponent's graveyard;
    /// `Triggering` is that player.
    W11cCrime {
        /// Who commits it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A player surveils ("whenever you surveil"); `Triggering` is that player.
    Surveilled {
        /// Who surveils.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
    },
    /// A step begins ("at the beginning of ...").
    StepBegins {
        /// The step.
        step: Step,
        /// Whose turn it is.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        whose: Ref,
    },
    /// A game state is true ("when you control no Islands", "whenever you have no cards in hand"; CR 603.8). The
    /// ability triggers when the state is true and not again until it has left the stack and the state is true
    /// again. It is the whole pattern of a triggered ability, never part of an `AnyOf`.
    State {
        /// The state.
        condition: Condition,
    },
    /// A player or object takes an action (`Action`): the one node for the keyword actions and the rest of the events
    /// that have no pattern of their own. `Triggering` is the player and the object, when the action has them.
    Did {
        /// What was done.
        action: Action,
        /// Who did it.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        who: Ref,
        /// The object it was done to or by; anything when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
    },
    /// Any of the patterns ("enters or attacks"); one event triggers the ability once.
    AnyOf(Vec<TriggerPattern>),
    /// One or more events the pattern describes that happen together ("one or more creatures you control deal combat
    /// damage to a player", "one or more cards are put into your graveyard from your library"): the events of one
    /// batch (one damage step, one effect) trigger the ability once, for each player they are about when they are
    /// about players. `Amount::Triggering` is how many events there were (the damage dealt for damage, the counters for
    /// counters). It is the whole pattern of a triggered ability, never part of an `AnyOf`.
    Batch(Box<TriggerPattern>),
    /// The nth time in a turn that an event happens to a player ("whenever you draw your second card each turn",
    /// "when you cast your first spell each turn"): the turn's count of that kind of event by that player, which
    /// starts again at zero when each turn begins. The event is `Drew`, `SpellCast`, `LifeGained`, `LifeLost`, `Cycled`
    /// or `Surveilled` of a player, a token created under your control (`ZoneChange` of `[Token, ControlledBy You]`
    /// from no zone) and, of an object, `Attacks`, `BecomesCrewed`, `BecomesSaddled`, `BecomesTarget`, `DealsDamage`
    /// into it and `CountersPut` on it.
    Nth {
        /// The event.
        event: Box<TriggerPattern>,
        /// Which time in the turn it must be, from one.
        nth: u32,
    },
    /// The event, when a spell or ability caused it ("whenever you put a -1/-1 counter on a creature", "when a spell or
    /// ability an opponent controls causes you to discard this"): the controller of the spell or ability that is
    /// resolving, being cast or activated when the event happens, or the controller of the source of the damage. Turn
    /// actions and state-based actions have no cause and never match.
    Caused {
        /// The event, or a `Batch` of events.
        inner: Box<TriggerPattern>,
        /// The players who caused it; any when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<Ref>,
        /// What caused it (the source of the spell or ability, or of the damage); anything when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        source: Selector,
    },
}

/// The event a replacement effect replaces (CR 614). Each variant is one kind of event the engine's replacement step
/// sees; `docs/cards-dsl.md` (Triggers, replacement and layers) says what each one matches.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ReplaceWhen {
    /// An effect would create tokens under the player's control; all the tokens one effect makes for a player are one event.
    TokenCreation {
        /// The player who would control them.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        controller: Ref,
        /// Only tokens of this kind ("one or more creature tokens"), judged by their types, subtypes, supertypes and
        /// colours; any token when empty. A run of tokens of several kinds is split by it.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
    },
    /// A source would deal damage to a player or a permanent.
    Damage {
        /// The recipient; any player or permanent when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<Ref>,
        /// The source of the damage; any source when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        source: Selector,
        /// Only combat damage (`true`) or only other damage (`false`); both when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        combat: Option<bool>,
    },
    /// An object matching the selector would move between zones, except a card drawn, a land played, a spell cast and
    /// a permanent entering (`Enters`).
    ZoneChange {
        /// The object that would move.
        what: Selector,
        /// The zone it would leave, if the pattern names one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<ZoneKind>,
        /// The zone it would enter, if the pattern names one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<ZoneKind>,
    },
    /// A permanent matching the selector would be destroyed (CR 701.8): by an effect that destroys, or by lethal damage or
    /// deathtouch; not sacrificed, put into a graveyard for zero toughness, or exiled.
    Destroyed {
        /// The permanent that would be destroyed.
        what: Selector,
    },
    /// A card matching the selector would be discarded (CR 701.9): the move from a hand to the graveyard that a discard is.
    Discarded {
        /// The card that would be discarded.
        what: Selector,
        /// Only a discard caused by a spell or ability these players control ("If a spell or ability an opponent
        /// controls causes you to discard this card"); any discard when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<Ref>,
    },
    /// A spell matching the selector would be countered (CR 701.6): the move from the stack to the graveyard of a spell an
    /// effect counted, not of one that resolved, fizzled or could not be countered.
    Countered {
        /// The spell that would be countered.
        what: Selector,
    },
    /// A face-down permanent matching the selector is turned face up ("As ~ is turned face up, put five +1/+1 counters
    /// on it", CR 708.8): the counters are put on it as it turns, before any player can respond. Only `WithCounters`
    /// on the permanent itself.
    TurnedFaceUp {
        /// The permanent turned face up.
        what: Selector,
    },
    /// Counters would be put on a permanent or a player, by an effect or as a permanent enters.
    CountersPut {
        /// What receives them; any permanent or player when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on: Option<Ref>,
        /// Only counters of this kind; any kind when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        counter: Option<CounterKind>,
        /// Only counters these players put: the controller of the spell or ability that puts them, or the player of
        /// the turn action; anyone when absent ("If you would put one or more counters").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<Ref>,
        /// Only counters a resolving spell or ability puts, not a turn action ("If an effect would put").
        #[serde(default, skip_serializing_if = "is_false")]
        effect: bool,
    },
    /// A player would gain life.
    LifeGain {
        /// Who would gain it.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// Only life a resolving spell or ability causes its own controller to gain ("If a spell or ability would cause
        /// its controller to gain life"); not the life of lifelink, which the damage causes.
        #[serde(default, skip_serializing_if = "is_false")]
        caused_own: bool,
    },
    /// A player would lose life: by an effect or by damage (CR 120.3a), not by paying it; only `LossTimes`.
    LifeLoss {
        /// Who would lose it.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// A player would lose the game; only `Instead`.
    LoseGame {
        /// Who.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// A player would mill one or more cards (CR 701.17): all the cards one effect mills from a player's library are
    /// one event, so `Multiply` and `Add` change how many are milled ("they mill twice that many cards instead").
    Mill {
        /// Who would mill.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// A player would draw a card.
    Draw {
        /// Who would draw.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// The first this many cards the player draws in each of their draw steps are not replaced ("except the first
        /// one you draw in each of your draw steps"); zero replaces every draw.
        #[serde(default, skip_serializing_if = "crate::spend::is_zero")]
        except_draw_step: u32,
    },
    /// A card matching the selector would enter the battlefield (a land played, a permanent spell resolving, a card
    /// put onto the battlefield); `This` is the card with the ability, in whatever zone it enters from.
    Enters {
        /// The permanent that would enter.
        what: Selector,
    },
    /// An Aura, Equipment or Fortification matching the selector would become attached to a permanent (CR 701.3): "As
    /// this Equipment becomes attached to a creature, choose a color." Only `Choose`; one attached again to the same
    /// permanent does not become attached, so nothing is chosen anew.
    BecomesAttached {
        /// The attachment.
        what: Selector,
    },
    /// A creature matching the selector would explore (CR 701.44); only `Instead`, whose `Explore` of `Triggering` is
    /// the replaced exploration ("instead it explores, then it explores again").
    Explores {
        /// The creature that would explore.
        what: Selector,
    },
    /// A creature matching the selector would connive (CR 701.50); only `Instead`, whose `Connive` of `Triggering` is
    /// the replaced connive ("instead you draw a card, then that creature connives").
    Connives {
        /// The creature that would connive.
        what: Selector,
    },
    /// A player would proliferate (CR 701.34); only `Instead`, whose `Proliferate` of that player is the replaced
    /// proliferation ("proliferate twice instead").
    Proliferates {
        /// Who would proliferate.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// A player would scry one or more cards (CR 701.22); only `Instead`, whose steps read the number as
    /// `Triggering` ("draw that many cards instead"). A scry of no cards does not happen, so nothing is replaced.
    Scries {
        /// Who would scry.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// A player would surveil one or more cards (CR 701.25); only `MayAddAmount` ("you may look at an additional two
    /// cards each time you surveil").
    Surveils {
        /// Who would surveil.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// A player would learn (CR 701.48); only `MayInstead` of returning the source from its owner's graveyard to the
    /// battlefield ("As long as this card is in your graveyard, if you would learn, you may instead return this card to
    /// the battlefield").
    Learns {
        /// Who would learn.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// A player would copy a spell one or more times (CR 707.10); only `AddAmount` of a fixed number ("instead copy it
    /// that many times plus an additional time"), and its controller may choose new targets for each added copy.
    CopiesSpell {
        /// Who would copy the spell.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
}

/// What a permanent's controller chooses as it enters the battlefield.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntersChoice {
    /// A colour.
    Color,
    /// A colour other than this one ("choose a color other than red").
    ColorOtherThan(Color),
    /// A creature type.
    CreatureType,
    /// A land type (CR 205.3i).
    LandType,
    /// A player.
    Player,
    /// An opponent of the permanent's controller.
    Opponent,
    /// A chapter number of the Saga, which it enters with as many lore counters (read ahead, CR 702.155).
    Chapter,
    /// A card name (CR 201.3): any card of the game.
    CardName,
    /// The name of a card that is not a land ("choose a nonland card name").
    NonlandCardName,
    /// The name of a card that is neither a creature nor a land ("choose a noncreature, nonland card name").
    NoncreatureNonlandCardName,
    /// The name of a land card that is not basic ("choose a nonbasic land card name").
    NonbasicLandCardName,
    /// Each opponent reveals their hand and the controller chooses the name of a nonland card revealed this way; no name
    /// when none was revealed.
    RevealedNonlandName,
    /// A card type.
    CardType,
    /// One of these card types ("choose artifact, creature, enchantment, instant, or sorcery").
    CardTypeOf(mtg_core::card::CardTypes),
    /// A basic land type (CR 205.3i: Plains, Island, Swamp, Mountain or Forest).
    BasicLandType,
    /// This many different colours ("choose two colors").
    Colors(u32),
    /// A colour and a creature type ("choose a color and a creature type").
    ColorAndCreatureType,
    /// This many different players ("choose two players").
    Players(u32),
    /// A number from `min` to `max`.
    Number {
        /// The smallest number.
        min: u32,
        /// The largest number.
        max: u32,
    },
}

/// Counters of one kind a permanent enters with besides the kind its replacement names first.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoreCounters {
    /// The kind of counter.
    pub counter: CounterKind,
    /// How many.
    pub count: Amount,
}

/// Counters a permanent entering as a copy enters with (`ReplaceBy::AsCopy`), when the object it copies matches `of`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopyCounters {
    /// The kind of counter.
    pub counter: CounterKind,
    /// How many.
    #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
    pub count: Amount,
    /// What the copied object must be, judged as it is copied; any object when empty.
    #[serde(default, skip_serializing_if = "Selector::is_empty")]
    pub of: Selector,
}

/// What happens instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ReplaceBy {
    /// The number or amount in the event is multiplied.
    Multiply(u32),
    /// The number or amount in the event is increased.
    Add(u32),
    /// The life lost is this many times as much (CR 614.1): "they lose twice that much life instead"; damage stays as it
    /// was dealt, only the life it takes changes. Only `LifeLoss`.
    LossTimes(u32),
    /// The number in the event is halved, rounded down ("they put half that many of each of those kinds of counters
    /// on that permanent or player instead, rounded down").
    Half,
    /// Damage minus this much (not a prevention).
    Subtract(u32),
    /// Damage over this much is this much (not a prevention).
    Cap(u32),
    /// The event does not happen (prevention, CR 615).
    Prevent,
    /// The next damage event it covers does not happen, and the shield ends ("the next time a source of your choice would
    /// deal damage to you this turn, prevent that damage"); only a shield uses it.
    PreventNext,
    /// The next damage event a shield covers does not happen, the effects follow it reading the damage prevented as the
    /// amount of the event (`Amount::Triggering`), and the shield ends ("the next time a source of your choice would deal
    /// damage to you this turn, prevent that damage. You gain life equal to the damage prevented this way"); only a shield
    /// uses it.
    PreventNextAnd(Vec<Effect>),
    /// Up to this much of the damage is prevented (CR 615). In a shield it is the total the shield prevents ("the next
    /// 3 damage"); in a static ability it is the most each event loses.
    PreventUpTo(u32),
    /// Up to this many of the damage in all, counted as the shield begins ("prevent the next X damage", CR 611.2c); only
    /// a shield uses it.
    PreventCounted(Amount),
    /// Up to `most` of the damage in all, counted as the shield begins, is prevented, and the effects follow each event
    /// it lessens, reading the damage prevented from it as the amount of the event (`Amount::Triggering`): "prevent the
    /// next 3 damage ... You gain life equal to the damage prevented this way"; only a shield uses it.
    PreventCountedAnd {
        /// The most damage prevented in all.
        most: Amount,
        /// What follows each prevention, reading the damage prevented.
        effects: Vec<Effect>,
    },
    /// Half of the damage is prevented, rounded up (CR 615): "prevent half that damage, rounded up"; only a static ability uses it.
    PreventHalf,
    /// These effects happen instead.
    Instead(Vec<Effect>),
    /// The tokens are created and the tokens these effects make with them, as one event, read as `Instead` reads it
    /// ("those tokens plus that many 1/1 green Squirrel creature tokens are created instead"); only a token creation uses it.
    Plus(Vec<Effect>),
    /// The next damage event a shield covers is replaced by these effects, as `Instead` reads it, and the shield ends
    /// ("the next time a source of your choice would deal damage to you this turn, that damage is dealt to X instead").
    InsteadNext(Vec<Effect>),
    /// Up to `most` of the damage is dealt to `to` instead, by the same source, and the rest as it was (CR 614.9); only a
    /// shield uses it, which fixes `to` as it begins and keeps the total it has left ("the next 1 damage that would be
    /// dealt to this creature this turn is dealt to target creature you control instead").
    RedirectUpTo {
        /// The most damage redirected in all.
        most: u32,
        /// The player or permanent the damage is dealt to instead.
        to: Ref,
    },
    /// `RedirectUpTo` with the total an amount counted as the shield begins (CR 611.2c): "the next X damage that would be
    /// dealt to ~ this turn is dealt to target creature instead"; only a shield uses it.
    RedirectCounted {
        /// The most damage redirected in all, counted as the shield begins.
        most: Amount,
        /// The player or permanent the damage is dealt to instead.
        to: Ref,
    },
    /// All of the damage is dealt to `to` instead, by the same source (CR 614.9); only a shield uses it, which fixes `to`
    /// as it begins ("all damage that would be dealt to ~ this turn is dealt to target creature instead"). With `next`,
    /// only the next damage event it covers, and the shield ends ("the next time damage would be dealt to ~ this turn").
    Redirect {
        /// The player or permanent the damage is dealt to instead.
        to: Ref,
        /// Only the next damage event.
        #[serde(default, skip_serializing_if = "is_false")]
        next: bool,
    },
    /// All of the damage is prevented (CR 615) and these effects happen, reading the event as `Instead` does ("prevent
    /// that damage and put that many +1/+1 counters on it").
    PreventAnd(Vec<Effect>),
    /// All but this much of the damage is prevented (CR 615): "prevent all but 1 of that damage".
    PreventAllBut(u32),
    /// All of the damage is prevented and the permanent with the ability loses one counter of this kind, if it has one
    /// ("prevent that damage. Remove a +1/+1 counter from this creature").
    PreventRemovingCounter(CounterKind),
    /// For each 1 damage, while the permanent with the ability has a counter of this kind, one such counter is removed
    /// and that 1 damage is prevented; damage that can't be prevented still removes the counters ("For each 1 damage that
    /// would be dealt to this creature, if it has a +1/+1 counter on it, remove a +1/+1 counter from it and prevent that
    /// 1 damage.").
    PreventByCounters(CounterKind),
    /// The next damage event a shield covers loses half of it, rounded down, and the shield ends ("the next time a source
    /// of your choice would deal damage to you this turn, prevent half that damage, rounded down"); only a shield uses it.
    PreventHalfNext,
    /// The next damage event a shield covers loses all but this much of it, and the shield ends ("the next time an
    /// unblocked creature of your choice would deal combat damage to you this turn, prevent all but 1 of that damage");
    /// only a shield uses it.
    PreventAllButNext(u32),
    /// A draw is skipped: the player mills this many cards and returns the card with the ability from its graveyard to
    /// their hand (dredge, CR 702.52a).
    Dredge(u32),
    /// The object goes to this zone instead ("exile it instead"); a card in a graveyard, a hand, the stack or exile
    /// goes to its owner's zone.
    MoveTo(ZoneKind),
    /// The object is put into its owner's library instead, at this place ("reveal it and shuffle it into its owner's library
    /// instead", "put it on top of its owner's library instead"); a reveal shows it to every player first.
    IntoLibrary {
        /// On top, on the bottom or shuffled in.
        place: Place,
        /// It is revealed as it moves.
        #[serde(default, skip_serializing_if = "is_false")]
        reveal: bool,
    },
    /// The permanent enters tapped (CR 614.1c).
    Tapped,
    /// The permanent enters untapped (CR 614.1c), also when another effect would have it enter tapped and is applied
    /// first ("Lands you control enter untapped").
    Untapped,
    /// The permanent enters with this many counters of the kind on it (CR 614.1c); they are put by an event of
    /// their own that other replacement effects can change.
    WithCounters {
        /// The kind of counter.
        counter: CounterKind,
        /// How many.
        count: Amount,
        /// It enters tapped as well ("enters tapped with two charge counters on it").
        #[serde(default, skip_serializing_if = "is_false")]
        tapped: bool,
    },
    /// The permanent enters with counters as `WithCounters` says, and has these changes for as long as it stays on the
    /// battlefield ("enters with two +1/+1 counters on it and with first strike").
    WithCountersAnd {
        /// The kind of counter.
        counter: CounterKind,
        /// How many.
        count: Amount,
        /// What it gets besides: a keyword or an ability.
        mods: Vec<Modification>,
        /// Counters of other kinds it enters with besides ("two +1/+1 counters and a trample counter on it").
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        also: Vec<MoreCounters>,
    },
    /// Amplify N (CR 702.38): as the permanent enters, its controller reveals any number of creature cards from their hand that
    /// share a creature type with it, and it enters with N +1/+1 counters for each.
    Amplify(u32),
    /// Tribute N (CR 702.104): as the permanent enters, an opponent of its controller chosen by the controller may put N +1/+1
    /// counters on it; it remembers whether they did as a number (1 paid, 0 not) that `Amount::Chosen` reads.
    Tribute(u32),
    /// The permanent enters with these counters when its controller says yes as it enters ("You may have this creature enter with a +1/+1 counter on it"); asked like the payment of `PayLifeOrTapped`.
    MayWithCounters {
        /// The kind of counter.
        counter: CounterKind,
        /// How many.
        count: Amount,
    },
    /// As the permanent enters, its controller makes this choice, which the permanent remembers for its abilities.
    Choose(EntersChoice),
    /// As the permanent enters, these effects happen first and then its controller makes the choice the permanent
    /// remembers (CR 614.1c): "As this artifact enters, look at an opponent's hand, then choose any card name."
    ChooseAfter {
        /// What happens before the choice.
        first: Vec<Effect>,
        /// What is chosen.
        choice: EntersChoice,
    },
    /// As the permanent enters, its controller may pay this much life; if they don't (or can't) it enters tapped.
    PayLifeOrTapped(u32),
    /// As the permanent enters, its controller may have it enter tapped; if they do, they get the counters ("you may have
    /// this land enter tapped. If you do, you get two rad counters").
    TappedForCounters {
        /// The kind of counter the player gets.
        counter: CounterKind,
        /// How many.
        count: u32,
    },
    /// As the permanent enters, its controller may reveal a card from their hand that the selector names; if they don't (or can't) it enters tapped.
    RevealOrTapped(Selector),
    /// As the permanent would enter, its controller pays the cost (a sacrifice or a discard); if they don't (or can't),
    /// it is put into its owner's graveyard instead ("If ~ would enter, sacrifice two untapped lands instead. If you do,
    /// put ~ onto the battlefield. If you don't, put it into its owner's graveyard").
    PayOrGraveyard {
        /// What is paid.
        cost: crate::cost::Cost,
        /// The controller may decline ("you may discard a land card instead").
        #[serde(default, skip_serializing_if = "is_false")]
        optional: bool,
    },
    /// Umbra armor (CR 702.89): all damage is removed from the permanent instead, and the object with the ability is destroyed.
    UmbraArmor,
    /// The permanent regenerates instead of being destroyed (CR 701.19): "If ~ would be destroyed, regenerate it." Not when
    /// it can't be regenerated; unlike a shield, it is not used up.
    Regenerate,
    /// As the permanent enters, its controller may have it enter as a copy of a permanent on the battlefield or a card
    /// in a graveyard that matches the selector (CR 707.9, 614.1c), with these exceptions in its copiable values
    /// ("except it's an Illusion in addition to its other types").
    AsCopy {
        /// The objects that may be copied: permanents, or cards when the selector names a graveyard.
        of: Selector,
        /// Changes the copy has: part of the copy effect, so part of its copiable values (CR 707.9b).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        except: Vec<Modification>,
        /// It enters tapped when it is a copy ("enter tapped as a copy of any land").
        #[serde(default, skip_serializing_if = "is_false")]
        tapped: bool,
        /// It is a copy until the end of the turn only ("becomes a copy ... until end of turn").
        #[serde(default, skip_serializing_if = "is_false")]
        until_end_of_turn: bool,
        /// It enters as a copy with no choice to decline whenever there is something to copy ("creatures you control
        /// enter as a copy of this creature").
        #[serde(default, skip_serializing_if = "is_false")]
        must: bool,
        /// Counters it enters with when it is a copy, each only when the copied object matches its selector ("except it
        /// enters with an additional +1/+1 counter on it if it's a creature").
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        counters: Vec<CopyCounters>,
    },
    /// Devour (CR 702.82): as the permanent enters, its controller may sacrifice any number of other permanents they control that match the selector, and it enters with this many +1/+1 counters for each.
    Devour {
        /// The +1/+1 counters for each permanent sacrificed; 0 is the number sacrificed for each ("Devour X").
        counters: u32,
        /// The permanents that may be sacrificed.
        of: Selector,
    },
    /// Riot (CR 702.136): as the permanent enters, its controller chooses a +1/+1 counter on it or haste for as long as they control it.
    Riot,
    /// As the permanent enters, it becomes day if it is neither day nor night (CR 726.2).
    DayIfNeither,
    /// The permanent enters with one counter of the kind its controller chooses among these, as it enters ("enters
    /// with your choice of a deathtouch counter or a lifelink counter on it").
    WithCounterOf(Vec<CounterKind>),
    /// The permanent enters with one counter of each of `count` different kinds its controller chooses among these, as it
    /// enters ("enters with your choice of two different counters on it from among menace, deathtouch, and lifelink").
    WithCountersOf {
        /// The kinds offered.
        kinds: Vec<CounterKind>,
        /// How many different kinds are chosen.
        count: u32,
    },
    /// The permanent enters prepared (a prepare card; its controller may then cast a copy of its spell face).
    EntersPrepared,
    /// As the permanent enters, its controller chooses one of these words ("choose Khans or Dragons"), which
    /// `Condition::EnteredAs` reads for as long as it stays.
    ChooseWord(Vec<String>),
    /// As the permanent enters, an opponent of its controller chooses what the kind names ("As ~ enters, an opponent
    /// chooses a creature type."): the controller chooses which opponent when there are several, and the choice is the
    /// permanent's as if its controller had made it. Nothing is chosen when there is no opponent.
    OpponentChooses(EntersChoice),
    /// The permanent enters under the control of an opponent its controller chooses as it enters ("~ enters under the
    /// control of an opponent of your choice"); under its controller's own control when there is no opponent.
    UnderChosenOpponent,
    /// As the permanent enters, it becomes its controller's choice of these forms ("it becomes your choice of a 3/3
    /// creature, a 2/2 creature with flying, or a 1/6 creature with defender"): the changes of the chosen form join its
    /// copiable values (CR 707.2), so a copy of it has them too. A form holds only changes a copy effect makes.
    BecomesChoice(Vec<Vec<Modification>>),
    /// As the permanent enters, its controller mills this many cards (CR 701.17): "As this creature enters, mill three
    /// cards for each time it was kicked."
    Mill(Amount),
    /// As the permanent enters, these effects happen first, then it enters (CR 614.1c, 614.12): "As ~ enters, discard
    /// your hand"; a question they ask goes to the permanent's controller.
    AsEnters(Vec<Effect>),
    /// As `AsEnters`, and then the permanent enters with `count` counters of the kind on it, the count worked out after the
    /// effects in the names they bound ("As ~ enters, remove all counters from all permanents. ~ enters with a +1/+1
    /// counter on it for each counter removed this way").
    AsEntersWith {
        /// What happens first.
        effects: Vec<Effect>,
        /// The kind of counter.
        counter: CounterKind,
        /// How many, read after the effects.
        count: Amount,
    },
    /// The number in the event is increased by an amount worked out as the event happens ("it deals that much damage
    /// plus X instead, where X is ~'s power"; `r304c_amounts`).
    AddAmount(Amount),
    /// Up to an amount worked out as the event happens of the damage is prevented ("prevent X of that damage, where X
    /// is the number of creatures you control"); only a static ability uses it.
    PreventAmount(Amount),
    /// All of the damage is prevented when it is this much or less ("If a source would deal 3 or less damage to ~,
    /// prevent that damage"); only a static ability uses it.
    PreventAtMost(u32),
    /// The controller of the replacement may add this many to the number of the action as it happens ("you may look at
    /// an additional two cards each time you surveil"); only a `Surveils` uses it.
    MayAddAmount(Amount),
    /// The permanent enters, or the tokens are created, under the control of the replacement's controller instead of an
    /// opponent's ("it enters under your control instead", "is created under your control instead"); only an `Enters`
    /// or a `TokenCreation` uses it, and only for what an opponent would get.
    UnderYourControl,
    /// The controller of the replacement may have these steps happen instead of the event, which then does not happen
    /// ("you may instead return this card to the battlefield"); only a `Learns` uses it.
    MayInstead(Vec<crate::effect::Effect>),
}

/// The kinds of shield (`Effect::Shield`) the engine plays; the validator and the engine both read a shield by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldShape {
    /// Objects named now that would go from the battlefield or the stack to a graveyard, or leave the battlefield, go
    /// elsewhere (CR 614.1a, 611.2c).
    Named {
        /// It covers any move from the battlefield, not only one to a graveyard.
        leaves: bool,
    },
    /// A spell named now that would be countered goes elsewhere.
    Countered,
    /// A class of moves or entries, judged as each happens.
    Class,
    /// Damage prevented, changed or redirected.
    Damage,
    /// The next draw, or the next loss of the game, of the players named is replaced by other steps, then the shield ends
    /// ("the next time you would draw a card this turn, ... instead", "the next time you would lose the game this turn,
    /// instead ..."; CR 614.1a).
    Next,
}

/// "<subject> becomes untapped", with " during your untap step" for an untap in the controller's untap step; an empty
/// subject gives the verb alone.
#[must_use]
pub fn untapped_words(subject: &str, untap_step: bool) -> String {
    let during = if untap_step {
        " during your untap step"
    } else {
        ""
    };
    if subject.is_empty() {
        format!("becomes untapped{during}")
    } else {
        format!("{subject} becomes untapped{during}")
    }
}

impl ShieldShape {
    /// The shape of a shield, or none when the engine plays no shield of this replacement.
    #[must_use]
    pub fn of(when: &ReplaceWhen, by: &ReplaceBy) -> Option<Self> {
        match (when, by) {
            (ReplaceWhen::ZoneChange { what, from, to }, ReplaceBy::MoveTo(_))
                if names_objects(what) =>
            {
                match (from, to) {
                    (Some(ZoneKind::Battlefield | ZoneKind::Stack), Some(ZoneKind::Graveyard)) => {
                        Some(Self::Named { leaves: false })
                    }
                    (Some(ZoneKind::Battlefield), None) => Some(Self::Named { leaves: true }),
                    _ => None,
                }
            }
            (ReplaceWhen::Countered { .. }, ReplaceBy::MoveTo(_))
            | (
                ReplaceWhen::Countered { .. },
                ReplaceBy::IntoLibrary {
                    place: Place::Top | Place::Bottom,
                    reveal: false,
                },
            ) => Some(Self::Countered),
            (ReplaceWhen::ZoneChange { what, .. }, ReplaceBy::MoveTo(_))
            | (ReplaceWhen::Enters { what }, ReplaceBy::Tapped | ReplaceBy::WithCounters { .. })
                if !names_objects(what) =>
            {
                Some(Self::Class)
            }
            // "Until end of turn, if one or more tokens would be created under your control, twice that many ..."
            // "If a creature would enter under an opponent's control this turn, it enters under your control instead."
            (
                ReplaceWhen::TokenCreation { .. },
                ReplaceBy::Multiply(_) | ReplaceBy::Add(_) | ReplaceBy::UnderYourControl,
            )
            // "Until end of turn, if target player would draw a card, instead ...": every draw of the players.
            | (ReplaceWhen::Draw { .. }, ReplaceBy::Instead(_)) => Some(Self::Class),
            (ReplaceWhen::Enters { what }, ReplaceBy::UnderYourControl) if !names_objects(what) => {
                Some(Self::Class)
            }
            (
                ReplaceWhen::Damage { .. },
                ReplaceBy::Prevent
                | ReplaceBy::PreventNext
                | ReplaceBy::PreventNextAnd(_)
                | ReplaceBy::PreventHalfNext
                | ReplaceBy::PreventAllButNext(_)
                | ReplaceBy::PreventUpTo(_)
                | ReplaceBy::PreventCounted(_)
                | ReplaceBy::PreventCountedAnd { .. }
                | ReplaceBy::PreventAnd(_)
                | ReplaceBy::Multiply(_)
                | ReplaceBy::Add(_)
                | ReplaceBy::Instead(_)
                | ReplaceBy::InsteadNext(_)
                | ReplaceBy::RedirectUpTo { .. }
                | ReplaceBy::RedirectCounted { .. }
                | ReplaceBy::Redirect { .. },
            ) => Some(Self::Damage),
            (
                ReplaceWhen::Draw { .. } | ReplaceWhen::LoseGame { .. },
                ReplaceBy::InsteadNext(_),
            ) => Some(Self::Next),
            _ => None,
        }
    }
}

/// Whether the selector names particular objects now (a target, what a spell damaged) rather than a kind of object.
#[must_use]
pub fn names_objects(what: &Selector) -> bool {
    use crate::refs::Pred;
    what.0
        .iter()
        .any(|pred| matches!(pred, Pred::Is(_) | Pred::SameCard(_) | Pred::DamagedBy(_)))
}

#[cfg(test)]
mod tests {
    //! Goal: patterns serialize compactly, leave out their defaults and refuse unknown fields.
    use super::*;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use crate::refs::Pred;

    #[rstest]
    #[case::enters(
        TriggerPattern::ZoneChange { what: Selector::this(), from: None, to: Some(ZoneKind::Battlefield) },
        r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#
    )]
    #[case::upkeep(
        TriggerPattern::StepBegins { step: Step::Upkeep, whose: Ref::You },
        r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#
    )]
    #[case::any_upkeep(
        TriggerPattern::StepBegins { step: Step::End, whose: Ref::EachPlayer },
        r#"{"StepBegins":{"step":"End"}}"#
    )]
    #[case::opponent_casts(
        TriggerPattern::SpellCast { what: Selector::default(), by: Ref::Opponents },
        r#"{"SpellCast":{"by":"Opponents"}}"#
    )]
    #[case::blocked_by_any(
        TriggerPattern::BecomesBlocked { what: Selector::this(), by: Selector::default(), once: false },
        r#"{"BecomesBlocked":{"what":["This"]}}"#
    )]
    #[case::blocked_by_a_kind(
        TriggerPattern::BecomesBlocked { what: Selector::this(), by: Selector(vec![Pred::Token]), once: false },
        r#"{"BecomesBlocked":{"what":["This"],"by":["Token"]}}"#
    )]
    #[case::a_state(
        TriggerPattern::State { condition: Condition::Not(Box::new(Condition::YourTurn)) },
        r#"{"State":{"condition":{"Not":"YourTurn"}}}"#
    )]
    #[case::either(
        TriggerPattern::AnyOf(vec![
            TriggerPattern::Attacks { what: Selector::this() },
            TriggerPattern::Sacrificed { what: Selector(vec![Pred::Type(CardType::Artifact)]), by: Ref::You },
        ]),
        r#"{"AnyOf":[{"Attacks":{"what":["This"]}},{"Sacrificed":{"what":[{"Type":"Artifact"}],"by":"You"}}]}"#
    )]
    fn patterns_have_stable_json(#[case] value: TriggerPattern, #[case] json: &str) {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(serde_json::from_str::<TriggerPattern>(json).unwrap(), value);
    }

    #[test]
    fn replacement_defaults_to_the_controller() {
        let when: ReplaceWhen = serde_json::from_str(r#"{"TokenCreation":{}}"#).unwrap();
        assert_eq!(
            when,
            ReplaceWhen::TokenCreation {
                controller: Ref::You,
                what: Selector::default(),
            }
        );
        assert_eq!(
            serde_json::to_string(&when).unwrap(),
            r#"{"TokenCreation":{}}"#
        );
    }

    #[test]
    fn an_unknown_field_in_a_struct_variant_is_an_error() {
        let error =
            serde_json::from_str::<TriggerPattern>(r#"{"Attacks":{"what":[],"alone":true}}"#)
                .unwrap_err()
                .to_string();
        assert!(error.contains("unknown field `alone`"), "{error}");
    }
}
