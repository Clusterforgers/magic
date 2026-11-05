//! Goal: round 9 piece 3 reads single-property gaps of cards in decks: base toughness and created tokens in valid-strings and the rest of the long tail.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given header and lines as compact JSON, and its gaps.
fn card(types: &str, lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::toughness("Creature.baseToughnessEQ1+YouCtrl", r#""stat":"BaseToughness""#)]
#[case::power("Creature.basePowerEQ1+YouCtrl", r#""stat":"BasePower""#)]
#[case::token_created("Card.tokenCreated+YouCtrl", r#""Token""#)]
fn a_valid_property_of_the_printed_card_or_its_making_is_read(
    #[case] valid: &str,
    #[case] expected: &str,
) {
    let (json, found) = card(
        "Artifact Equipment",
        &format!(
            "S:Mode$ Continuous | AffectedDefined$ Equipped | Affected$ Creature | AddPower$ X | AddToughness$ X\nSVar:X:Count$Valid {valid}\nK:Equip:2"
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn a_token_made_this_turn_gates_an_activation() {
    let (json, found) = card(
        "Artifact",
        "A:AB$ Draw | Cost$ T | NumCards$ 1 | CheckSVar$ X\nSVar:X:Count$ThisTurnEntered_Battlefield_Card.tokenCreated+YouCtrl",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains("Token"), "{json}");
}

#[rstest]
#[case::poisoned("True", true)]
#[case::not_asked("False", false)]
fn an_attack_trigger_asks_whether_the_defender_is_poisoned(
    #[case] flag: &str,
    #[case] asked: bool,
) {
    let (json, found) = card(
        "Creature Rat",
        &format!(
            "PT:2/2\nT:Mode$ Attacks | ValidCard$ Creature.Self | TriggerZones$ Battlefield | DefendingPlayerPoisoned$ {flag} | Execute$ P\nSVar:P:DB$ Pump | NumAtt$ +1 | NumDef$ +1 | Defined$ Self"
        ),
    );
    assert_eq!(found.is_empty(), asked, "{found:?}");
    assert_eq!(json.contains(r#""on":"Defending""#), asked, "{json}");
}
