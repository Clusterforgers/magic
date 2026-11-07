//! Goal: round 7 Differs faults stay closed: an enters-as-a-copy replacement with a later step (Spark Double's extra counter) is a gap, and an AnimateAll over the creatures of a target player is a gap, while the shapes without them still translate.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The gaps of a card with the given type line and lines.
fn card_gaps(types: &str, lines: &str) -> Vec<String> {
    let text = format!("Name:Subject\nTypes:{types}\nPT:0/0\n{lines}\n");
    let _ = abilities(&text);
    gaps(&text)
}

const COPY: &str = "K:ETBReplacement:Copy:DBCopy:Optional\nSVar:DBCopy:DB$ Clone | Choices$ Creature.Other+YouCtrl | NonLegendary$ True";

#[rstest]
#[case::copy_alone("", false)]
#[case::copy_then_more(
    " | SubAbility$ DBPut\nSVar:DBPut:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1",
    true
)]
fn r7a_a_copy_with_a_later_step_is_a_gap(#[case] more: &str, #[case] gap: bool) {
    let gaps = card_gaps("Creature Shapeshifter", &format!("{COPY}{more}"));
    assert_eq!(
        gaps.iter()
            .any(|gap| gap.contains("Clone.replacement-SubAbility")),
        gap,
        "{gaps:?}"
    );
}

#[rstest]
#[case::all_creatures("", false)]
#[case::target_players(" | ValidTgts$ Player", true)]
fn r7a_b_animate_all_of_a_target_player_is_a_gap(#[case] target: &str, #[case] gap: bool) {
    let lines = format!(
        "A:SP$ AnimateAll | ValidCards$ Creature | Power$ 0 | Toughness$ 2 | RemoveAllAbilities$ True{target} | SpellDescription$ Until end of turn, creatures lose all abilities and have base power and toughness 0/2."
    );
    let gaps = card_gaps("Instant", &lines);
    assert_eq!(
        gaps.iter()
            .any(|gap| gap.contains("AnimateAll.ValidTgts=Player")),
        gap,
        "{gaps:?}"
    );
}
