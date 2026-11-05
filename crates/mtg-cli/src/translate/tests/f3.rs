//! Goal: the werewolf upkeep words are table rows over the spells cast last turn: transform after none, and back after one player cast two or more.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

#[rstest]
#[case::transform(
    "WerewolfTransformCondition",
    json!({"Compare": {"left": {"CastLastTurn": "EachPlayer"}, "op": "Eq", "right": 0}})
)]
#[case::untransform(
    "WerewolfUntransformCondition",
    json!({"Compare": {"left": {"CastLastTurn": "EachPlayer"}, "op": "Ge", "right": 2}})
)]
fn a_werewolf_word_is_a_condition_on_the_spells_of_the_last_turn(
    #[case] word: &str,
    #[case] condition: Value,
) {
    let script = format!(
        "Name:T\nTypes:Creature Human Werewolf\nPT:2/2\nT:Mode$ Phase | Phase$ Upkeep | {word}$ True | TriggerZones$ Battlefield | Execute$ Turn | TriggerDescription$ At the beginning of each upkeep, transform.\nSVar:Turn:DB$ SetState | Defined$ Self | Mode$ Transform\n"
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    let ability = &abilities(&script)[0]["Triggered"];
    assert_eq!(ability["if"], condition);
    assert_eq!(ability["when"], json!({"StepBegins": {"step": "Upkeep"}}));
}
