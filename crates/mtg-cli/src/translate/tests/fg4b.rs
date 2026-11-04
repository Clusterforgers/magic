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
