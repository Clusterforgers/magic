//! Goal: the mappers of the fg4b family turn invented Forge-format lines into the DSL nodes the engine has and leave the rest as gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn trigger(line: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nT:{line} | TriggerZones$ Battlefield | Execute$ Go\nSVar:Go:DB$ Draw | NumCards$ 1\n"
    )
}

#[rstest]
#[case::scry("Mode$ Scry | ValidPlayer$ You", json!({"action": "Scried", "who": "You"}))]
#[case::surveil_any("Mode$ Surveil | ValidPlayer$ Player", json!({"action": "Surveilled"}))]
#[case::shuffled("Mode$ Shuffled | ValidPlayer$ Opponent", json!({"action": "Shuffled", "who": "Opponents"}))]
#[case::lost("Mode$ LosesGame | ValidPlayer$ You", json!({"action": "LostGame", "who": "You"}))]
#[case::land("Mode$ LandPlayed | ValidCard$ Land.YouCtrl", json!({"action": "PlayedLand", "what": [{"Type": "Land"}, {"ControlledBy": "You"}]}))]
#[case::explores("Mode$ Explores | ValidCard$ Card.Self", json!({"action": "Explored", "what": ["This"]}))]
#[case::monstrous("Mode$ BecomeMonstrous | ValidCard$ Card.Self", json!({"action": "BecameMonstrous", "what": ["This"]}))]
#[case::mutates("Mode$ Mutates | ValidCard$ Creature.YouCtrl", json!({"action": "Mutated", "what": [{"Type": "Creature"}, {"ControlledBy": "You"}]}))]
fn action_modes_are_did(#[case] line: &str, #[case] node: Value) {
    let expected = normal(json!({"Triggered": {"when": {"Did": node}, "effects": [{"Draw": {}}]}}));
    assert_eq!(abilities(&trigger(line)), vec![expected]);
}

#[rstest]
#[case::cause(
    "Mode$ Countered | ValidCard$ Card | ValidCause$ Card.Self",
    "key:Trigger.Countered.ValidCause"
)]
#[case::own_library(
    "Mode$ SearchedLibrary | ValidPlayer$ You | SearchOwnLibrary$ True",
    "key:Trigger.SearchedLibrary.SearchOwnLibrary"
)]
#[case::result(
    "Mode$ RolledDie | ValidPlayer$ You | ValidResult$ 6",
    "key:Trigger.RolledDie.ValidResult"
)]
fn extra_keys_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&trigger(line)), vec![gap.to_owned()]);
}

#[rstest]
#[case::set_life("SP$ SetLife | Defined$ You | LifeAmount$ 20", json!({"SetLife": {"amount": 20}}))]
#[case::set_life_of_a_target("SP$ SetLife | ValidTgts$ Player | LifeAmount$ 1", json!({"SetLife": {"who": {"Target": 0}, "amount": 1}}))]
#[case::lose_game("SP$ LosesGame | ValidTgts$ Opponent", json!({"LoseGame": {"who": {"Target": 0}}}))]
#[case::win_game("SP$ WinsGame | Defined$ You", json!({"WinGame": {}}))]
#[case::end_turn("SP$ EndTurn", json!("EndTurn"))]
#[case::amass("SP$ Amass | Type$ Orc | Num$ 2", json!({"Amass": {"subtype": "Orc", "count": 2}}))]
#[case::goad("SP$ Goad | ValidTgts$ Creature.OppCtrl", json!({"Goad": {"what": {"Target": 0}}}))]
#[case::connive("SP$ Connive | Defined$ Self", json!({"Connive": {"what": "This"}}))]
#[case::venture("SP$ Venture | Defined$ You", json!({"Venture": {}}))]
fn game_verbs(#[case] line: &str, #[case] effect: Value) {
    let script = format!("Name:T\nTypes:Sorcery\nA:{line}\n");
    let spell = &abilities(&script)[0]["Spell"];
    assert_eq!(spell["effects"], json!([effect]), "{spell}");
}

#[rstest]
#[case::exert("2 Exert<1/CARDNAME>", json!([{"Mana": "{2}"}, "Exert"]))]
#[case::blight("Blight<2>", json!([{"Blight": 2}]))]
#[case::damage("DamageYou<3>", json!([{"DamageToYou": 3}]))]
#[case::evidence("CollectEvidence<4>", json!([{"CollectEvidence": 4}]))]
#[case::exile_top("ExileFromTop<2/Card>", json!([{"ExileTop": 2}]))]
#[case::forage("T Forage", json!(["Tap", "Forage"]))]
#[case::add_counter("AddCounter<1/M1M1>", json!([{"PutCounters": {"counter": "-1/-1"}}]))]
#[case::remove_from_a_creature("SubCounter<1/P1P1/Creature.YouCtrl/a creature you control>", json!([{"RemoveCounters": {"counter": "+1/+1", "from": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}]))]
#[case::remove_any_from_self("RemoveAnyCounter<1/Any/CARDNAME/this creature>", json!([{"RemoveCounters": {}}]))]
#[case::remove_any_among("RemoveAnyCounter<2/Any/Creature.YouCtrl/creatures>", json!([{"RemoveCountersAmong": {"count": 2, "from": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}]))]
fn cost_words(#[case] cost: &str, #[case] expected: Value) {
    let script = format!("Name:T\nTypes:Artifact\nA:AB$ Draw | Cost$ {cost} | NumCards$ 1\n");
    assert_eq!(abilities(&script)[0]["Activated"]["cost"], expected);
}

#[rstest]
#[case::goad_duration(
    "SP$ Goad | ValidTgts$ Creature | Duration$ Permanent",
    "key:Goad.Duration"
)]
#[case::connive_count(
    "SP$ Connive | Defined$ Self | ConniveNum$ X",
    "key:Connive.ConniveNum"
)]
#[case::redistribute(
    "SP$ SetLife | Defined$ You | LifeAmount$ 1 | Redistribute$ True",
    "key:SetLife.Redistribute"
)]
fn game_verb_extras_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    let script = format!("Name:T\nTypes:Sorcery\nA:{line}\n");
    assert_eq!(gaps(&script), vec![gap.to_owned()]);
}
