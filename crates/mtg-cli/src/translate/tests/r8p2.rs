//! Goal: the entering-the-battlefield replacements of the card itself read their Forge shapes right, and a shape the DSL cannot say stays a named gap.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn land(line: &str, svars: &str) -> String {
    format!("Name:T\nTypes:Land\n{line}\n{svars}")
}

const REVEAL_LINE: &str = "R:Event$ Moved | ValidCard$ Card.Self | Destination$ Battlefield | ReplaceWith$ DBReveal | ReplacementResult$ Updated";

#[rstest]
#[case::dragon(
    "Pirate",
    "Count$Valid Pirate.inZoneBattlefield+YouCtrl",
    "EQ0",
    Some(json!({"Replacement": {"when": {"Enters": {"what": ["This"]}},
        "by": {"RevealOrTapped": [{"Subtype": "Pirate"}, {"In": "Hand"}, {"OwnedBy": "You"}]},
        "condition": {"Not": {"Compare": {"left": {"Count": [{"Subtype": "Pirate"}, {"In": "Battlefield"}, {"ControlledBy": "You"}]}, "op": "Ge", "right": 1}}}}}))
)]
#[case::compare_is_not_zero("Pirate", "Count$Valid Pirate.inZoneBattlefield+YouCtrl", "GE1", None)]
#[case::count_is_not_a_sum("Pirate", "Count$Valid Pirate.inZoneBattlefield+YouCtrl", "EQ0", None)]
fn reveal_a_card_or_enter_tapped(
    #[case] valid: &str,
    #[case] held: &str,
    #[case] compare: &str,
    #[case] expected: Option<Value>,
) {
    let presence = if expected.is_none() && compare == "EQ0" {
        held.to_owned()
    } else {
        format!("Count$RememberedSize/Plus.Held\nSVar:Held:{held}")
    };
    let svars = format!(
        "SVar:DBReveal:DB$ Reveal | RevealValid$ {valid} | Optional$ True | RememberRevealed$ True | SubAbility$ DBTap\nSVar:DBTap:DB$ Tap | ETB$ True | Defined$ Self | ConditionCheckSVar$ Presence | ConditionSVarCompare$ {compare} | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:Presence:{presence}\n"
    );
    let text = land(REVEAL_LINE, &svars);
    match expected {
        Some(expected) => {
            assert_eq!(gaps(&text), Vec::<String>::new());
            assert_eq!(abilities(&text)[0], expected);
        }
        None => assert!(
            gaps(&text)
                .iter()
                .any(|gap| gap.starts_with("replacement:Moved-Reveal")),
            "{:?}",
            gaps(&text)
        ),
    }
}
