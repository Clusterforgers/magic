//! Goal: every card the r111-03 pack made automatic reads back as its printed text clause by clause, and a near miss of
//! each (another number, a dropped condition, another reference), replaced wherever it occurs, does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn report_of(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(text, card, &tokens, &names(&cards), &Caps::everything())
}

/// The rules file of a corpus card as compact JSON.
fn compact(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    serde_json::to_string(&value).unwrap()
}

#[rstest]
#[case("Arboria")]
#[case("Territorial Hellkite")]
#[case("Suffocation")]
#[case("Backdraft")]
#[case("Glyph of Reincarnation")]
#[case("Booby Trap")]
#[case("Null Chamber")]
#[case("Medomai's Prophecy")]
#[case("Phyrexian Grimoire")]
fn r111_03_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(report.verdict(), Verdict::Accepted, "{name}: {report:#?}");
}

#[rstest]
#[case::any_permanent(
    "Arboria",
    r#""R111CastOrPutLastTurn""#,
    r#"{"Not":"R111CastOrPutLastTurn"}"#
)]
#[case::the_wrong_controller("Suffocation", r#""amount":4"#, r#""amount":3"#)]
#[case::rounded_up("Backdraft", r#""up":false"#, r#""up":true"#)]
#[case::another_damage("Booby Trap", r#""amount":10"#, r#""amount":9"#)]
#[case::three_from_the_top("Phyrexian Grimoire", r#""count":2"#, r#""count":3"#)]
#[case::the_named_draw("Booby Trap", r#""what":["ChosenName"]"#, r#""what":[]"#)]
#[case::any_spell("Medomai's Prophecy", r#""Eq","right":1"#, r#""Ge","right":1"#)]
#[case::attacked_before(
    "Territorial Hellkite",
    r#"{"Not":{"R111AttackedLastCombat":"This"}}"#,
    r#"{"R111AttackedLastCombat":"This"}"#
)]
fn r111_03_a_near_miss_is_not_accepted(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let text = compact(name);
    assert!(text.contains(from), "{name} holds {from}");
    let near = text.replace(from, to);
    assert_ne!(
        report_of(name, &near).verdict(),
        Verdict::Accepted,
        "{near}"
    );
}
