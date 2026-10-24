//! Goal: the lines of a die table ("1—9 | S", "20 | S") are read into the roll of the clause before them, a table is read whole or not at all (one row that does not read leaves the roll and every row to the table), rows out of order or after an open row stay unread, and a proposed rules file with a table is read back row by row.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::card::ImplStatus;
use mtg_cards::compile::compile;
use mtg_cards::effect::Effect;
use mtg_cards::load::Library;
use mtg_cards::proposal::{ClauseState, Verdict, check};
use mtg_cards::support::{PrintedCard, PrintedFace};
use rstest::rstest;

use crate::common::{printed, token_sources};

const ID: &str = "043d8dee-379f-4706-ad47-0bb562717007";

fn creature(text: &str) -> PrintedCard {
    PrintedCard {
        layout: "normal".into(),
        oracle_id: ID.into(),
        name: "Test Roller".into(),
        keywords: vec![],
        faces: vec![PrintedFace {
            name: "Test Roller".into(),
            type_line: "Creature \u{2014} Elemental".into(),
            oracle_text: text.into(),
            ..PrintedFace::default()
        }],
    }
}

/// The rows of the table of the first roll an ability holds, as (from, to).
fn table(abilities: &[Ability]) -> Vec<(u32, Option<u32>)> {
    abilities
        .iter()
        .flat_map(|ability| ability.effects())
        .find_map(|effect| match effect {
            Effect::RollDice { results, .. } => {
                Some(results.iter().map(|row| (row.from, row.to)).collect())
            }
            _ => None,
        })
        .unwrap_or_default()
}

const ROLL: &str = "When this creature enters, roll a d20.";

#[rstest]
#[case::three_rows(
    "1\u{2014}9 | Each player sacrifices a permanent of their choice.\n10\u{2014}19 | Each opponent sacrifices a permanent of their choice.\n20 | Each opponent sacrifices two permanents of their choice.",
    vec![(1, Some(9)), (10, Some(19)), (20, Some(20))]
)]
#[case::an_open_end(
    "1\u{2014}14 | Each player sacrifices a permanent of their choice.\n15+ | Each opponent sacrifices a permanent of their choice.",
    vec![(1, Some(14)), (15, None)]
)]
#[case::one_row(
    "1\u{2014}20 | Each opponent sacrifices a permanent of their choice.",
    vec![(1, Some(20))]
)]
fn every_row_of_a_table_joins_the_roll_before_it(
    #[case] rows: &str,
    #[case] expected: Vec<(u32, Option<u32>)>,
) {
    let compiled = compile(&creature(&format!("{ROLL}\n{rows}")));
    let made = compiled.card.as_ref().expect("the card compiles");
    assert_eq!(made.impl_status, ImplStatus::Full, "{compiled:?}");
    assert!(compiled.clauses.iter().all(|clause| clause.read));
    assert_eq!(made.faces[0].abilities.len(), 1);
    assert_eq!(table(&made.faces[0].abilities), expected);
}

#[rstest]
#[case::a_row_that_does_not_read(
    "1\u{2014}9 | Each player sacrifices a permanent of their choice.\n10\u{2014}20 | Each opponent frobnicates."
)]
#[case::rows_out_of_order(
    "10\u{2014}20 | Each opponent sacrifices a permanent of their choice.\n1\u{2014}9 | Each player sacrifices a permanent of their choice."
)]
#[case::overlapping_rows(
    "1\u{2014}10 | Each player sacrifices a permanent of their choice.\n10\u{2014}20 | Each opponent sacrifices a permanent of their choice."
)]
#[case::a_row_after_an_open_end(
    "15+ | Each player sacrifices a permanent of their choice.\n16\u{2014}20 | Each opponent sacrifices a permanent of their choice."
)]
#[case::a_row_with_a_target("1\u{2014}20 | Destroy target creature.")]
fn a_table_with_a_row_that_does_not_join_is_left_whole_to_the_table(#[case] rows: &str) {
    let compiled = compile(&creature(&format!("{ROLL}\n{rows}")));
    let made = compiled.card.as_ref().expect("the card compiles");
    assert_ne!(made.impl_status, ImplStatus::Full, "{compiled:?}");
    assert!(table(&made.faces[0].abilities).is_empty(), "{compiled:?}");
    assert!(
        compiled
            .clauses
            .iter()
            .filter(|clause| clause.text.contains(" | ") || clause.text == ROLL)
            .all(|clause| !clause.read),
        "{compiled:?}"
    );
}

#[test]
fn rows_after_a_clause_that_does_not_roll_stay_unread() {
    let compiled = compile(&creature(
        "When this creature enters, draw a card.\n1\u{2014}20 | Each opponent sacrifices a permanent of their choice.",
    ));
    let rows = compiled
        .clauses
        .iter()
        .filter(|clause| clause.text.contains(" | "))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].read, "{compiled:?}");
}

#[test]
fn a_roll_without_rows_is_a_roll_alone() {
    let compiled = compile(&creature(ROLL));
    let made = compiled.card.as_ref().expect("the card compiles");
    assert_eq!(made.impl_status, ImplStatus::Full, "{compiled:?}");
    assert!(table(&made.faces[0].abilities).is_empty());
}

const ELEMENTAL: &str = r#"{"faces":[{"abilities":[{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"RollDice":{"sides":20,"results":[{"from":1,"to":9,"effects":[{"Sacrifice":{"who":"EachPlayer","filter":[]}}]},{"from":10,"to":19,"effects":[{"Sacrifice":{"who":"Opponents","filter":[]}}]},{"from":20,"to":20,"effects":[{"Sacrifice":{"who":"Opponents","filter":[],"count":2}}]}]}}]}}]}]}"#;

fn proposal(rules: &str) -> mtg_cards::proposal::Report {
    let cards = printed();
    let tokens = Library::from_sources(&token_sources()).unwrap();
    let names: BTreeSet<String> = cards.iter().map(|c| c.name.to_lowercase()).collect();
    let card = cards
        .iter()
        .find(|card| card.name == "Earth-Cult Elemental")
        .expect("the corpus has the card");
    check(rules, card, &tokens, &names, &Caps::everything())
}

#[test]
fn a_proposal_with_the_table_of_the_card_is_read_back_row_by_row() {
    let report = proposal(ELEMENTAL);
    assert_eq!(report.verdict(), Verdict::Accepted, "{report:?}");
    assert!(
        report
            .clauses
            .iter()
            .all(|clause| matches!(clause.state, ClauseState::Reads { .. })),
        "{report:?}"
    );
}

#[rstest]
#[case::another_row_effect(
    r#""effects":[{"Sacrifice":{"who":"Opponents","filter":[]}}]},{"from":20"#,
    r#""effects":[{"Sacrifice":{"who":"EachPlayer","filter":[]}}]},{"from":20"#
)]
#[case::another_range(r#"{"from":10,"to":19"#, r#"{"from":10,"to":18"#)]
fn a_row_the_text_words_differently_is_a_mismatch(#[case] from: &str, #[case] to: &str) {
    assert!(ELEMENTAL.contains(from), "{from}");
    let report = proposal(&ELEMENTAL.replace(from, to));
    assert_eq!(report.verdict(), Verdict::Mismatch, "{report:?}");
    assert!(
        report
            .clauses
            .iter()
            .any(|clause| matches!(clause.state, ClauseState::Differs { .. })),
        "{report:?}"
    );
}
