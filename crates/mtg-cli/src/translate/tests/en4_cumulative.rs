//! Goal: a cumulative upkeep line becomes the keyword with its cost, and a cost the cost words cannot say stays a named gap.
use rstest::rstest;

use super::{abilities, gaps};

#[rstest]
#[case::mana("Cumulative upkeep:1", "{1}")]
#[case::coloured("Cumulative upkeep:U", "{U}")]
#[case::described_life("Cumulative upkeep:PayLife<2>:Pay 2 life.", "ife")]
#[case::sacrifice("Cumulative upkeep:Sac<1/Creature>:Sacrifice a creature.", "acrifice")]
fn cumulative_upkeep_maps(#[case] line: &str, #[case] has: &str) {
    let script = format!("Name:T\nTypes:Creature Elf\nK:{line}\n");
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let text = found[0].to_string();
    assert!(
        text.contains("CumulativeUpkeep") && text.contains(has),
        "{text}"
    );
}

#[rstest]
#[case::draw("Cumulative upkeep:Draw<1/You>:Draw a card.")]
fn a_cost_without_a_cost_item_is_a_gap(#[case] line: &str) {
    let script = format!("Name:T\nTypes:Creature Elf\nK:{line}\n");
    assert_eq!(abilities(&script).len(), 0);
    assert_eq!(gaps(&script), ["cost:Draw"]);
}
