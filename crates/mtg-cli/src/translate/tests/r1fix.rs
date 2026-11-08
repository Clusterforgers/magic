//! Goal: the readings the round 1 audit found wrong are right or a named gap: a card put into a graveyard this turn, "any opponent may sacrifice", "any other target", a target the script narrows to your permanents, and taking one of the cards looked at.
#![allow(clippy::unwrap_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};
use crate::translate::any_player::narrows_target;

fn json_of(text: &str) -> String {
    serde_json::to_string(&abilities(text)).unwrap()
}

#[test]
fn a_card_put_into_a_graveyard_this_turn_is_selected_by_its_arrival_in_any_zone() {
    let text = "Name:Harvester\nTypes:Creature Demon\nPT:1/1\nA:AB$ ChangeZone | Cost$ T | Origin$ Graveyard | Destination$ Exile | ValidTgts$ Creature.ThisTurnEntered | TgtZone$ Graveyard\nOracle:{T}: Exile target creature card from a graveyard that was put there this turn.";
    assert_eq!(gaps(text), Vec::<String>::new());
    let json = json_of(text);
    assert!(json.contains(r#"{"ArrivedFrom":"Library"}"#), "{json}");
    assert!(!json.contains("EnteredThisTurn"), "{json}");
}

#[test]
fn a_permanent_that_entered_this_turn_keeps_its_reading() {
    let text = "Name:Entered\nTypes:Creature Demon\nPT:1/1\nA:AB$ Destroy | Cost$ T | ValidTgts$ Creature.ThisTurnEntered\nOracle:{T}: Destroy target creature that entered this turn.";
    assert!(json_of(text).contains(r#""EnteredThisTurn""#));
}

const DEMON: &str = "Name:Demon\nTypes:Creature Demon\nPT:6/6\nT:Mode$ Phase | Phase$ BeginCombat | TriggerZones$ Battlefield | Execute$ TrigSac\nSVar:TrigSac:DB$ Sacrifice | Defined$ Opponent | Amount$ 1 | SacValid$ Creature | RememberSacrificed$ True | Optional$ True | SubAbility$ DBSacSelf\nSVar:DBSacSelf:DB$ Tap | Defined$ Self | ConditionCheckSVar$ X | SubAbility$ DBPutCounter\nSVar:DBPutCounter:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1 | ConditionCheckSVar$ X | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Remembered$Amount\nOracle:At the beginning of each combat, any opponent may sacrifice a creature. If a player does, tap Demon and put a +1/+1 counter on it.";

#[test]
fn any_opponent_may_sacrifice_is_a_payment_asked_in_turn_with_the_steps_after_it_paid() {
    assert_eq!(gaps(DEMON), Vec::<String>::new());
    let found = abilities(DEMON);
    let effects = &found[0]["Triggered"]["effects"];
    assert_eq!(effects.as_array().unwrap().len(), 1, "{effects}");
    let pay = &effects[0]["Pay"];
    assert_eq!(pay["who"], json!("Opponents"));
    assert_eq!(
        pay["cost"],
        json!([{"Sacrifice": {"filter": [{"Type": "Creature"}]}}])
    );
    assert_eq!(pay["paid"].as_array().unwrap().len(), 2, "{pay}");
}

#[test]
fn a_may_sacrifice_by_one_player_stays_a_may() {
    let text = DEMON.replace("Defined$ Opponent", "Defined$ You");
    let json = json_of(&text);
    assert!(
        json.contains(r#""May""#) && !json.contains(r#""Pay""#),
        "{json}"
    );
}

#[test]
fn any_other_target_after_a_creature_excludes_that_creature() {
    let text = "Name:Bond\nTypes:Sorcery\nA:SP$ GainControl | ValidTgts$ Creature | LoseControl$ EOT | SubAbility$ DBDamage\nSVar:DBDamage:DB$ DealDamage | ValidTgts$ Any | NumDmg$ 2 | TargetUnique$ True\nOracle:Gain control of target creature until end of turn. Bond deals 2 damage to any other target.";
    assert_eq!(gaps(text), Vec::<String>::new());
    let found = abilities(text);
    let second = &found[0]["Spell"]["targets"][1];
    let json = second.to_string();
    assert!(
        json.contains("Either") && json.contains(r#"{"Not":{"Is":{"Target":0}}}"#),
        "{second}"
    );
    assert!(!json.contains("AnyTarget"), "{second}");
}

#[rstest]
#[case::says_nothing(
    "Target creature can't be blocked this turn.",
    "Creature.YouCtrl",
    true
)]
#[case::you_control(
    "Target creature you control can't be blocked.",
    "Creature.YouCtrl",
    false
)]
#[case::opponent(
    "Destroy target creature an opponent controls.",
    "Creature.OppCtrl",
    false
)]
#[case::attacking("Target attacking creature gets +1/+1.", "Creature.YouCtrl", false)]
#[case::no_controller_in_the_script("Target creature can't be blocked.", "Creature", false)]
#[case::no_target_phrase("Creatures can't be blocked.", "Creature.YouCtrl", false)]
fn a_controller_the_script_adds_to_a_target_is_a_gap(
    #[case] oracle: &str,
    #[case] valid: &str,
    #[case] narrower: bool,
) {
    assert_eq!(narrows_target(oracle, valid), narrower);
}

const GONTI: &str = "Name:Gonti\nTypes:Creature Aetherborn\nPT:2/3\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigDig\nSVar:TrigDig:DB$ Dig | ValidTgts$ Opponent | DigNum$ 4 | ChangeNum$ 1 | DestinationZone$ Exile | DestinationZone2$ Library | LibraryPosition$ -1 | ExileFaceDown$ True | WithMayLook$ True | ChangeValid$ Card | RememberChanged$ True\n";

#[rstest]
#[case::mandatory(
    "When Gonti enters, look at the top four cards of target opponent's library, exile one of them face down, then put the rest on the bottom.",
    false
)]
#[case::may_in_the_next_sentence(
    "When Gonti enters, look at the top four cards of target opponent's library. You may exile one of them face down.",
    true
)]
#[case::up_to(
    "When Gonti enters, look at the top four cards of your library, exile up to one of them.",
    true
)]
fn taking_a_card_looked_at_may_be_none_only_when_the_text_says_so(
    #[case] oracle: &str,
    #[case] up_to: bool,
) {
    let text = format!("{GONTI}Oracle:{oracle}");
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    let choose = found[0]["Triggered"]["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|step| step.get("Choose"))
        .cloned()
        .unwrap_or(Value::Null);
    assert_eq!(choose["count"]["min"] == json!(0), up_to, "{choose}");
}

#[test]
fn a_sized_token_is_written_with_its_size_in_the_token() {
    let text = "Name:Sizer\nTypes:Sorcery\nA:SP$ Token | TokenPower$ X | TokenToughness$ X | TokenScript$ w_x_x_spirit | TokenOwner$ You\nSVar:X:Count$Valid Creature.YouCtrl\n";
    let json = json_of(text);
    assert!(
        json.contains(r#""Sized""#) && !json.contains("SetPT"),
        "{json}"
    );
}
