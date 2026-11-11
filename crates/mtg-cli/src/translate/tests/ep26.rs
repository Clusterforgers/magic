//! Goal: the pieces of ep26 translate as the card prints them: a target chosen at random (`TargetsAtRandom$ True`) on any verb that reads targets, over players, opponents and objects; a random number of random targets stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn activated(line: &str) -> String {
    format!("Name:T\nTypes:Artifact Creature Goblin\nPT:1/1\nA:AB$ {line}\n")
}

fn targets(script: &str) -> Value {
    abilities(script)
        .first()
        .and_then(|found| found.get("Activated"))
        .and_then(|body| body.get("targets"))
        .cloned()
        .unwrap_or_default()
}

#[rstest]
#[case::any_target(
    "DealDamage | Cost$ T | NumDmg$ 2 | ValidTgts$ Any | TargetsAtRandom$ True",
    json!("AnyTarget")
)]
#[case::an_opponent(
    "DealDamage | Cost$ T | NumDmg$ 2 | ValidTgts$ Opponent | TargetsAtRandom$ True",
    json!("Opponent")
)]
#[case::a_creature_an_opponent_controls(
    "Destroy | Cost$ T | ValidTgts$ Creature.OppCtrl | TargetsAtRandom$ True",
    json!({"Object": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]})
)]
fn ep26_a_target_chosen_at_random(#[case] line: &str, #[case] what: Value) {
    let script = activated(line);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{line}");
    assert_eq!(targets(&script), json!([{"what": {"AtRandom": what}}]));
}

#[rstest]
#[case::a_random_number_of_targets(
    "Tap | Cost$ T | ValidTgts$ Creature | TargetMin$ 0 | TargetMax$ 3 | TargetsAtRandom$ True"
)]
#[case::another_word("DealDamage | Cost$ T | NumDmg$ 2 | ValidTgts$ Any | TargetsAtRandom$ Some")]
fn ep26_other_random_targets_stay_gaps(#[case] line: &str) {
    let found = gaps(&activated(line));
    assert!(
        found.iter().any(|gap| gap.contains("TargetsAtRandom")),
        "{line}: {found:?}"
    );
}
