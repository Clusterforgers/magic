//! Patterns over game events: what triggers an ability and what a replacement effect replaces.

use mtg_core::color::Color;
use serde::{Deserialize, Serialize};

use crate::effect::{Effect, is_false};
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
/// engine events (`docs/cards-dsl.md`, Triggers).
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
    /// A permanent matching the selector becomes untapped.
    Untapped {
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
    },
    /// A creature matching the selector blocks.
    Blocks {
        /// The blocker.
        what: Selector,
    },
    /// A creature matching the selector becomes blocked.
    BecomesBlocked {
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
    /// Counters are put on a permanent.
    CountersPut {
        /// What receives them; any permanent when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
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
    /// An Aura, Equipment or Fortification becomes attached to a permanent.
    Attached {
        /// The attachment; any when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// The permanent it becomes attached to; any when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        to: Selector,
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
    /// A player proliferates ("whenever you proliferate"); `Triggering` is that player.
    Proliferated {
        /// Who proliferates.
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
    /// Any of the patterns ("enters or attacks"); one event triggers the ability once.
    AnyOf(Vec<TriggerPattern>),
    /// One or more events the pattern describes that happen together ("one or more creatures you control deal combat
    /// damage to a player", "one or more cards are put into your graveyard from your library"): the events of one
    /// batch (one damage step, one effect) trigger the ability once, for each player they are about when they are
    /// about players. `Amount::Triggering` is how many events there were. It is the whole pattern of a triggered
    /// ability, never part of an `AnyOf`.
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
    },
    /// Counters would be put on a permanent or a player, by an effect or as a permanent enters.
    CountersPut {
        /// What receives them; any permanent or player when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on: Option<Ref>,
        /// Only counters of this kind; any kind when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        counter: Option<CounterKind>,
    },
    /// A player would gain life.
    LifeGain {
        /// Who would gain it.
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
    /// A player.
    Player,
    /// An opponent of the permanent's controller.
    Opponent,
}

/// What happens instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ReplaceBy {
    /// The number or amount in the event is multiplied.
    Multiply(u32),
    /// The number or amount in the event is increased.
    Add(u32),
    /// The event does not happen (prevention, CR 615).
    Prevent,
    /// Up to this much of the damage is prevented (CR 615). In a shield it is the total the shield prevents ("the next
    /// 3 damage"); in a static ability it is the most each event loses.
    PreventUpTo(u32),
    /// These effects happen instead.
    Instead(Vec<Effect>),
    /// All of the damage is prevented and the permanent with the ability loses one counter of this kind, if it has one
    /// ("prevent that damage. Remove a +1/+1 counter from this creature").
    PreventRemovingCounter(CounterKind),
    /// A draw is skipped: the player mills this many cards and returns the card with the ability from its graveyard to
    /// their hand (dredge, CR 702.52a).
    Dredge(u32),
    /// The object goes to this zone instead ("exile it instead"); a card in a graveyard, a hand, the stack or exile
    /// goes to its owner's zone.
    MoveTo(ZoneKind),
    /// The permanent enters tapped (CR 614.1c).
    Tapped,
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
    /// As the permanent enters, its controller makes this choice, which the permanent remembers for its abilities.
    Choose(EntersChoice),
    /// As the permanent enters, its controller may pay this much life; if they don't (or can't) it enters tapped.
    PayLifeOrTapped(u32),
    /// As the permanent enters, its controller may reveal a card from their hand that the selector names; if they don't (or can't) it enters tapped.
    RevealOrTapped(Selector),
    /// Umbra armor (CR 702.89): all damage is removed from the permanent instead, and the object with the ability is destroyed.
    UmbraArmor,
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
    },
    /// Devour (CR 702.82): as the permanent enters, its controller may sacrifice any number of other permanents they control that match the selector, and it enters with this many +1/+1 counters for each.
    Devour {
        /// The +1/+1 counters for each permanent sacrificed.
        counters: u32,
        /// The permanents that may be sacrificed.
        of: Selector,
    },
    /// Riot (CR 702.136): as the permanent enters, its controller chooses a +1/+1 counter on it or haste for as long as they control it.
    Riot,
    /// As the permanent enters, it becomes day if it is neither day nor night (CR 726.2).
    DayIfNeither,
    /// The permanent enters prepared (a prepare card; its controller may then cast a copy of its spell face).
    EntersPrepared,
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
                controller: Ref::You
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
