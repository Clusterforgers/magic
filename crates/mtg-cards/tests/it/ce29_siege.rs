//! Goal: "as long as you control at least N <kind>" reads as "N or more" does and is written back in its own words, for several numbers and kinds, while "at least one" stays with the table; Invasion of Theros reads whole on both faces.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::compile::compile;
use mtg_cards::support::{PrintedCard, PrintedFace};
use rstest::rstest;

use crate::common::printed;

fn creature(text: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: String::new(),
        name: "Probe".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Probe".into(),
            type_line: "Creature".into(),
            oracle_text: text.into(),
            ..PrintedFace::default()
        }],
    }
}

#[rstest]
#[case::three_other_enchantments(
    "~ has lifelink and indestructible as long as you control at least three other enchantments.",
    true
)]
#[case::two_artifacts("~ has flying as long as you control at least two artifacts.", true)]
#[case::the_or_more_wording("~ has flying as long as you control two or more artifacts.", true)]
#[case::at_least_one("~ has flying as long as you control at least one artifact.", false)]
fn at_least_reads_as_or_more(#[case] text: &str, #[case] reads: bool) {
    let compiled = compile(&creature(text));
    assert_eq!(
        compiled.clauses.iter().all(|clause| clause.read),
        reads,
        "{text}"
    );
}

#[test]
fn invasion_of_theros_reads_whole() {
    let card = printed()
        .into_iter()
        .find(|card| card.name.starts_with("Invasion of Theros"))
        .unwrap();
    let compiled = compile(&card);
    let unread: Vec<&str> = compiled
        .clauses
        .iter()
        .filter(|clause| !clause.read)
        .map(|clause| clause.text.as_str())
        .collect();
    assert!(unread.is_empty(), "{unread:?}");
}
