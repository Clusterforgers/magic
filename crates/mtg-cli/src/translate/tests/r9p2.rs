//! Goal: round 9 piece 2 reads parameters that several effects share, so one rule closes each family.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a creature with the given lines as compact JSON, and its gaps.
fn creature(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Creature Elf\nPT:1/1\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::lands("Land", "3")]
#[case::creatures("Creature.YouCtrl", "X")]
fn untap_up_to_picks_at_most_the_amount(#[case] kind: &str, #[case] n: &str) {
    let (json, found) = creature(&format!(
        "A:AB$ Untap | Cost$ T | UntapUpTo$ True | UntapType$ {kind} | Amount$ {n}\nSVar:X:Count$xPaid"
    ));
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(r#""min":0"#) && json.contains("Untap"),
        "{json}"
    );
}

#[rstest]
#[case::minus_on_mine("Creature.YouCtrl", "M1M1", r#""PutCounterOf":"-1/-1""#)]
#[case::plus_on_theirs("Creature.OppCtrl", "P1P1", r#""PutCounterOf":"+1/+1""#)]
fn a_ban_on_one_kind_of_counter_names_the_kind(
    #[case] valid: &str,
    #[case] kind: &str,
    #[case] expected: &str,
) {
    let (json, found) = creature(&format!(
        "S:Mode$ CantPutCounter | ValidCard$ {valid} | CounterType$ {kind}"
    ));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn a_ban_on_poison_counters_on_you_names_the_kind() {
    let (json, found) = creature("S:Mode$ CantPutCounter | ValidPlayer$ You | CounterType$ POISON");
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""PutCounterOf":"poison""#), "{json}");
}
