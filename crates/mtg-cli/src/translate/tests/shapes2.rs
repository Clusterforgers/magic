//! Goal: a cost the card words as "another" or "other" keeps that word whatever the script wrote, and a cost worded both ways is a gap.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;

use super::{abilities, gaps};

fn permanent(cost: &str, oracle: &str) -> String {
    format!(
        "Name:Subject\nManaCost:2\nTypes:Creature Elf\nPT:2/2\nA:AB$ Pump | Cost$ {cost} | Defined$ Self | NumAtt$ +1\nOracle:{}\n",
        oracle.replace('\n', "\\n")
    )
}

#[test]
fn the_same_cost_worded_both_ways_is_a_gap() {
    let text = permanent(
        "Sac<1/Creature>",
        "Sacrifice another creature: x.\nSacrifice a creature: y.",
    );
    assert_eq!(gaps(&text), vec!["cost:Other-ambiguous".to_string()]);
}

#[rstest]
#[case::tap_another(
    "tapXType<2/Creature>",
    "Tap two other untapped creatures you control: Subject gets +1/+0.",
    true
)]
#[case::tap_another_one(
    "tapXType<1/Creature>",
    "{1}, {T}, Tap another untapped creature you control: Subject gets +1/+0.",
    true
)]
#[case::script_already_says_other("tapXType<2/Creature.Other>", "Tap two other creatures.", true)]
#[case::no_other_word(
    "tapXType<2/Creature>",
    "Tap two untapped creatures you control: x.",
    false
)]
#[case::other_is_somewhere_else(
    "tapXType<2/Creature>",
    "Tap two creatures you control: Another creature gets +1/+0.",
    false
)]
#[case::sacrifice_another(
    "Sac<1/Creature>",
    "Sacrifice another creature: Subject gets +1/+0.",
    true
)]
#[case::sacrifice_a("Sac<1/Creature>", "Sacrifice a creature: Subject gets +1/+0.", false)]
#[case::another_kind_of_permanent_elsewhere(
    "Sac<1/Land>",
    "Sacrifice another creature: x.\nSacrifice a Swamp: Subject gets +1/+0.",
    false
)]
#[case::another_is_one_of_three_counts(
    "tapXType<3/Creature>",
    "Tap another untapped creature you control: x.\nTap three untapped creatures you control: Subject gets +1/+0.",
    false
)]
#[case::each_other_is_no_cost(
    "Sac<1/Creature>",
    "Sacrifice each other creature you control.",
    false
)]
fn a_cost_keeps_the_other_the_card_says(
    #[case] cost: &str,
    #[case] oracle: &str,
    #[case] other: bool,
) {
    let text = permanent(cost, oracle);
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    let paid = found[0]["Activated"]["cost"][0].to_string();
    assert_eq!(paid.contains("Other"), other, "{paid}");
}
