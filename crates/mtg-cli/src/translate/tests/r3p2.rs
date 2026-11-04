//! Goal: round 3 piece P2 (choices and loops): next-turn delayed triggers and the Discard modes.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn later(phase: &str) -> String {
    format!(
        "Name:T\nTypes:Instant\nA:SP$ DelayedTrigger | NextTurn$ True | Mode$ Phase | Phase$ {phase} | ValidPlayer$ Player | Execute$ Draw\nSVar:Draw:DB$ Draw | Defined$ You\n"
    )
}

/// Arcane Denial's first trigger: nobody holds priority before the upkeep begins, so the next upkeep is the next turn's.
#[test]
fn a_next_turn_upkeep_is_the_next_upkeep() {
    let found = abilities(&later("Upkeep"));
    assert_eq!(
        found[0]["Spell"]["effects"],
        json!([{"DelayedTrigger": {"when": {"StepBegins": {"step": "Upkeep"}},
            "effects": [{"Draw": {}}]}}])
    );
}

/// A later step could still come this turn, so it stays a gap by name.
#[rstest]
#[case::draw("Draw")]
#[case::end("End of Turn")]
#[case::main("Main1")]
fn a_next_turn_later_step_is_a_gap(#[case] phase: &str) {
    assert_eq!(gaps(&later(phase)), ["key:DelayedTrigger.NextTurn"]);
}
