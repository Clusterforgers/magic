//! Goal: an `Effect` object with statics and no remembered objects maps to the continuous effects or rules of its
//! statics for its duration, and a static that is more than a continuous change stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

#[rstest]
#[case::pump_all_you_control(
    "Name:T\nTypes:Sorcery\nA:SP$ Effect | StaticAbilities$ Pump\nSVar:Pump:Mode$ Continuous | Affected$ Creature.YouCtrl | AddPower$ 1 | AddKeyword$ Haste\n",
    json!({"Spell": {"effects": [{"Continuous": {
        "affected": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]},
        "mods": [{"ModifyPT": {"power": 1, "toughness": 0}}, {"AddKeyword": "Haste"}],
        "duration": "UntilEndOfTurn"}}]}})
)]
#[case::two_statics_for_the_rest_of_the_game(
    "Name:T\nTypes:Sorcery\nA:SP$ Effect | StaticAbilities$ A,B | Duration$ Permanent\nSVar:A:Mode$ Continuous | Affected$ Creature.YouCtrl | AddPower$ 1\nSVar:B:Mode$ Continuous | Affected$ Creature.OppCtrl | AddToughness$ -1\n",
    json!({"Spell": {"effects": [
        {"Continuous": {"affected": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]},
            "mods": [{"ModifyPT": {"power": 1, "toughness": 0}}], "duration": "Forever"}},
        {"Continuous": {"affected": {"Each": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]},
            "mods": [{"ModifyPT": {"power": 0, "toughness": -1}}], "duration": "Forever"}}]}})
)]
fn ambient_statics_map(#[case] script: &str, #[case] expected: Value) {
    let mut found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    assert_eq!(normal(found.remove(0)), normal(expected));
}

#[rstest]
#[case::unknown_mode("Mode$ Quux | Affected$ Creature.YouCtrl")]
#[case::missing_static("Mode$ Continuous | Affected$ Creature.YouCtrl | AddPower$ 1 | Quux$ 1")]
fn other_ambient_statics_stay_gaps(#[case] static_line: &str) {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Effect | StaticAbilities$ S\nSVar:S:{static_line}\n"
    );
    assert!(abilities(&script).is_empty(), "{script}");
    assert!(!gaps(&script).is_empty());
}
