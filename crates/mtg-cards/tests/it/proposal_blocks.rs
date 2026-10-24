//! Goal: the levels of a leveler, a Class and a Spacecraft are paired with the abilities that work under their level, each is written back without the condition the block adds, and a block whose header, size, line or range differs from the text does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common::{cards_dir, printed, token_sources};

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{ClauseState, Report, Verdict, check};
use rstest::rstest;

fn run(name: &str, rules: &str) -> Report {
    let cards = printed();
    let tokens = Library::from_sources(&token_sources()).unwrap();
    let names: BTreeSet<String> = cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect();
    let card = cards.iter().find(|card| card.name == name).unwrap();
    check(rules, card, &tokens, &names, &Caps::everything())
}

/// The hand-written rules of a card as compact JSON.
fn file_of(card: &str) -> String {
    let text = std::fs::read_to_string(cards_dir().join(format!("{}.json", slug(card)))).unwrap();
    serde_json::from_str::<serde_json::Value>(&text)
        .unwrap()
        .to_string()
}

#[rstest]
#[case::a_leveler("Echo Mage")]
#[case::a_leveler_with_keywords("Student of Warfare")]
#[case::a_class("Wizard Class")]
#[case::a_class_with_a_trigger("Stormchaser's Talent")]
#[case::a_spacecraft("Galvanizing Sawship")]
#[case::a_spacecraft_with_an_activation("Debris Field Crusher")]
#[case::atmospheric_greenhouse("Atmospheric Greenhouse")]
#[case::caravan_escort("Caravan Escort")]
#[case::coralhelm_commander("Coralhelm Commander")]
#[case::hada_spy_patrol("Hada Spy Patrol")]
#[case::hearthhull("Hearthhull, the Worldseed")]
#[case::kazandu_tuskcaller("Kazandu Tuskcaller")]
#[case::lumen_class_frigate("Lumen-Class Frigate")]
#[case::rescue_skiff("Rescue Skiff")]
#[case::susurian_dirgecraft("Susurian Dirgecraft")]
#[case::the_seriema("The Seriema")]
#[case::uthros_research_craft("Uthros Research Craft")]
#[case::uthros_scanship("Uthros Scanship")]
#[case::wedgelight_rammer("Wedgelight Rammer")]
#[case::wurmwall_sweeper("Wurmwall Sweeper")]
fn the_levels_of_a_card_read_back_without_the_condition_they_add(#[case] card: &str) {
    let report = run(card, &file_of(card));

    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{}",
        report.render(true)
    );
}

/// The rules of a card with `from` replaced by `to` everywhere.
fn replaced(card: &str, from: &str, to: &str) -> String {
    let text = file_of(card);
    assert!(text.contains(from), "{card} has no {from}");
    text.replace(from, to)
}

/// The rules of a card with the ability at `at` of its first face removed.
fn without(card: &str, at: usize) -> String {
    let mut rules: serde_json::Value = serde_json::from_str(&file_of(card)).unwrap();
    rules["faces"][0]["abilities"]
        .as_array_mut()
        .unwrap()
        .remove(at);
    rules.to_string()
}

/// The rules of a card with the ability at `at` of its first face written twice.
fn doubled(card: &str, at: usize) -> String {
    let mut rules: serde_json::Value = serde_json::from_str(&file_of(card)).unwrap();
    let list = rules["faces"][0]["abilities"].as_array_mut().unwrap();
    let again = list[at].clone();
    list.push(again);
    rules.to_string()
}

#[rstest]
#[case::a_size_that_differs(
    "Echo Mage",
    replaced(
        "Echo Mage",
        r#""power":2,"toughness":4"#,
        r#""power":3,"toughness":4"#
    ),
    Verdict::Mismatch
)]
#[case::a_copy_count_that_differs(
    "Echo Mage",
    replaced("Echo Mage", r#""count":2"#, r#""count":3"#),
    Verdict::Mismatch
)]
#[case::a_range_that_differs(
    "Echo Mage",
    replaced("Echo Mage", r#""op":"Le","right":3"#, r#""op":"Le","right":4"#),
    Verdict::Unchecked
)]
#[case::a_keyword_the_card_does_not_print(
    "Student of Warfare",
    replaced(
        "Student of Warfare",
        r#""AddKeyword":"FirstStrike""#,
        r#""AddKeyword":"DoubleStrike""#
    ),
    Verdict::Invalid
)]
#[case::a_level_up_cost_that_differs(
    "Wizard Class",
    replaced("Wizard Class", r#""cost":"{2}{U}""#, r#""cost":"{3}{U}""#),
    Verdict::Mismatch
)]
#[case::a_level_that_differs(
    "Wizard Class",
    replaced(
        "Wizard Class",
        r#""BecomeLevel":{"level":2}"#,
        r#""BecomeLevel":{"level":3}"#
    ),
    Verdict::Mismatch
)]
#[case::a_count_that_differs(
    "Wizard Class",
    replaced("Wizard Class", r#""Draw":{"count":2}"#, r#""Draw":{"count":3}"#),
    Verdict::Mismatch
)]
#[case::a_line_left_out("Echo Mage", without("Echo Mage", 2), Verdict::Mismatch)]
#[case::a_size_left_out("Echo Mage", without("Echo Mage", 1), Verdict::Mismatch)]
#[case::a_level_up_left_out("Wizard Class", without("Wizard Class", 1), Verdict::Mismatch)]
#[case::a_line_the_text_lacks("Echo Mage", doubled("Echo Mage", 2), Verdict::Mismatch)]
fn a_block_that_differs_from_the_text_does_not_read_back(
    #[case] card: &str,
    #[case] rules: String,
    #[case] verdict: Verdict,
) {
    let report = run(card, &rules);

    assert_eq!(report.verdict(), verdict, "{}", report.render(true));
}

#[test]
fn a_block_of_a_proposal_that_models_none_of_it_is_left_to_the_table() {
    let mut rules: serde_json::Value = serde_json::from_str(&file_of("Echo Mage")).unwrap();
    rules["faces"][0]["abilities"] = serde_json::json!([{ "Keyword": { "LevelUp": "{1}{U}" } }]);

    let report = run("Echo Mage", &rules.to_string());

    assert_eq!(report.verdict(), Verdict::Accepted);
    let open = report
        .clauses
        .iter()
        .filter(|clause| matches!(clause.state, ClauseState::Open))
        .count();
    assert_eq!(open, 2, "{}", report.render(true));
}
