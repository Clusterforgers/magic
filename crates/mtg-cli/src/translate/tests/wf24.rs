//! Goal: the translator faults the semantic audit found stay closed: a group of a target opponent's creatures reads the
//! target (Curious Colossus), two targets of one `MoveCounter` line are distinct objects (Nexus Mentality), and the
//! keywords a lasting `GainControl` adds last as long as the text says (Slave of Bolas, Momo's Heist).
#![allow(clippy::unwrap_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(lines: &str) -> String {
    format!("Name:Subject\nTypes:Sorcery\n{lines}\n")
}

#[rstest]
#[case::target_opponent(
    "ValidTgts$ Opponent | ValidCards$ Creature",
    Some(json!({"Each": [{"Type": "Creature"}, {"ControlledBy": {"Target": 0}}]}))
)]
#[case::no_target("ValidCards$ Creature", Some(json!({"Each": [{"Type": "Creature"}]})))]
#[case::target_object("ValidTgts$ Creature | ValidCards$ Creature", None)]
fn wf24_an_animated_group_reads_its_target(#[case] keys: &str, #[case] affected: Option<Value>) {
    let text = spell(&format!(
        "A:SP$ AnimateAll | {keys} | Power$ 1 | Toughness$ 1 | Duration$ Permanent | SpellDescription$ X."
    ));
    match affected {
        Some(affected) => {
            let found = &abilities(&text)[0]["Spell"];
            assert_eq!(found["effects"][0]["Continuous"]["affected"], affected);
            let targeted = affected.to_string().contains("Target");
            assert_eq!(found.get("targets").is_some(), targeted, "{found}");
        }
        None => assert!(
            gaps(&text)
                .iter()
                .any(|gap| gap.contains("AnimateAll.ValidTgts")),
            "{:?}",
            gaps(&text)
        ),
    }
}

#[test]
fn wf24_two_targets_of_one_move_are_distinct() {
    let text = spell(
        "A:SP$ MoveCounter | TargetMin$ 2 | TargetMax$ 2 | ValidTgts$ Permanent.nonLand+YouCtrl | CounterType$ All | CounterNum$ All | SpellDescription$ X.",
    );
    let targets = &abilities(&text)[0]["Spell"]["targets"];
    let first = targets[0]["what"]["Object"].as_array().unwrap();
    let second = targets[1]["what"]["Object"].as_array().unwrap();
    assert!(!first.contains(&json!({"Not": {"Is": {"Target": 1}}})));
    assert_eq!(second.last(), Some(&json!({"Not": {"Is": {"Target": 0}}})));
}

#[rstest]
#[case::until_end_of_turn(
    "",
    "Gain control of target creature. It gains haste until end of turn.",
    Some("UntilEndOfTurn")
)]
#[case::as_long_as_control(
    "",
    "Gain control of target artifact. It gains haste.",
    Some("Forever")
)]
#[case::control_ends(
    " | LoseControl$ EOT",
    "Until end of turn, gain control of target creature. It gains haste until end of turn.",
    Some("UntilEndOfTurn")
)]
#[case::two_durations("", "Until end of turn, X. It gains haste until end of turn.", None)]
fn wf24_added_keywords_last_as_the_text_says(
    #[case] lose: &str,
    #[case] description: &str,
    #[case] lasts: Option<&str>,
) {
    let text = spell(&format!(
        "A:SP$ GainControl | ValidTgts$ Creature | AddKWs$ Haste{lose} | SpellDescription$ {description}"
    ));
    match lasts {
        Some(lasts) => {
            let effects = &abilities(&text)[0]["Spell"]["effects"];
            assert_eq!(
                effects[1]["Continuous"]["mods"],
                json!([{"AddKeyword": "Haste"}])
            );
            assert_eq!(effects[1]["Continuous"]["duration"], json!(lasts));
        }
        None => assert!(
            gaps(&text)
                .iter()
                .any(|gap| gap.contains("GainControl.AddKWs-durations")),
            "{:?}",
            gaps(&text)
        ),
    }
}
