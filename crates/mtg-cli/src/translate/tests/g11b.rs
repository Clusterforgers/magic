//! Goal: round 11 pack g11b reads the damage words of Forge scripts: damage that can't be prevented (`NoPrevention$`)
//! keeps the flag on its damage, and an unknown value of it or a redirection limit beside it stays a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// Asserts that the card reads `expected` (and none of `absent`) with no gap, or stays the gap `gap` when one is given.
fn reads(types: &str, lines: &str, expected: &[&str], absent: &[&str], gap: &str) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    let found = gaps(&text);
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    if gap.is_empty() {
        assert!(found.is_empty(), "{found:?}");
        for part in expected {
            assert!(json.contains(part), "{part} in {json}");
        }
        for part in absent {
            assert!(!json.contains(part), "{part} not in {json}");
        }
    } else {
        assert!(found.iter().any(|g| g.contains(gap)), "{found:?} {json}");
    }
}

#[rstest]
#[case::cant_be_prevented(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4 | NoPrevention$ True",
    &[r#""unpreventable":true"#],
    &[],
    ""
)]
#[case::plain_damage(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4",
    &[r#""amount":4"#],
    &["unpreventable"],
    ""
)]
#[case::false_is_plain(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4 | NoPrevention$ False",
    &[r#""amount":4"#],
    &["unpreventable"],
    ""
)]
#[case::an_unknown_value(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4 | NoPrevention$ Combat",
    &[],
    &[],
    "key:NoPrevention=Combat"
)]
#[case::no_redirection_stays_a_gap(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4 | NoPrevention$ True | NoRedirection$ True",
    &[],
    &[],
    "NoRedirection"
)]
fn damage_words(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] expected: &[&str],
    #[case] absent: &[&str],
    #[case] gap: &str,
) {
    reads(types, lines, expected, absent, gap);
}
