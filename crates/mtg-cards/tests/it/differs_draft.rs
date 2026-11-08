//! Goal: a draft marked as reading back as other text (`differs`) runs only on a player's read: never as `Forge`, pending until read, `Read` once approved, read again after a change, listed to read in a deck with its differing clauses; an unmarked one never runs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use crate::common::{Corpus, printed};

use mtg_cards::card::Card;
use mtg_cards::deck::{CardFacts, DeckLine, DeckZone, Legality, deck_report};
use mtg_cards::load::Library;
use mtg_cards::support::{Caps, PrintedCard, READBACK_DIFFERS, Support, read_hash};
use mtg_cards::{DraftRules, ReadState};
use mtg_core::color::ColorSet;
use rstest::rstest;

const BOLT: &str = "Lightning Bolt";

/// Lightning Bolt's printed card and rules that deal `amount` damage: any amount but 3 reads back as other text.
fn bolt(amount: i64) -> (Card, PrintedCard) {
    let card = Corpus::repo()
        .library()
        .card_by_name(BOLT)
        .expect("the corpus has Lightning Bolt")
        .clone();
    let mut rules = serde_json::to_value(&card).unwrap();
    rules["faces"][0]["abilities"][0]["Spell"]["effects"][0]["DealDamage"]["amount"] =
        amount.into();
    let shown = printed()
        .into_iter()
        .find(|card| card.name == BOLT)
        .expect("the corpus has Lightning Bolt");
    (serde_json::from_value(rules).unwrap(), shown)
}

/// A library whose only card is a draft of Bolt dealing `amount`, its verdicts given on `read_on`'s hash.
fn library(
    amount: i64,
    read_on: i64,
    read: ReadState,
    forge: bool,
    differs: bool,
) -> (Library, PrintedCard) {
    let (card, shown) = bolt(amount);
    let (read_card, _) = bolt(read_on);
    let row = DraftRules {
        oracle_id: card.oracle_id.clone(),
        read_hash: read_hash(&shown, &read_card),
        rules: serde_json::to_value(&card).unwrap(),
        read,
        forge,
        differs,
        proof: None,
    };
    let (library, left_out) = Library::default().with_drafts(&[row]);
    assert!(left_out.is_empty(), "{left_out:?}");
    (library, shown)
}

#[rstest]
#[case::a_differs_draft_without_a_verdict_even_from_forge(
    2,
    2,
    ReadState::Pending,
    true,
    true,
    Support::Manual
)]
#[case::a_differs_draft_the_server_calls_approved_on_trust_is_read_not_forge(
    2,
    2,
    ReadState::Approved,
    true,
    true,
    Support::Read
)]
#[case::one_correct_verdict_makes_a_differs_draft_read(
    2,
    2,
    ReadState::Approved,
    false,
    true,
    Support::Read
)]
#[case::a_changed_rule_needs_a_new_read(1, 2, ReadState::Approved, false, true, Support::Manual)]
#[case::a_differs_draft_found_wrong(2, 2, ReadState::Wrong, false, true, Support::Manual)]
#[case::an_unmarked_draft_that_differs_never_runs(
    2,
    2,
    ReadState::Approved,
    false,
    false,
    Support::Manual
)]
#[case::the_forge_trust_still_applies_to_a_draft_that_reads_back(
    3,
    3,
    ReadState::Approved,
    true,
    false,
    Support::Forge
)]
fn a_draft_that_differs_runs_on_a_players_read_alone(
    #[case] amount: i64,
    #[case] read_on: i64,
    #[case] read: ReadState,
    #[case] forge: bool,
    #[case] differs: bool,
    #[case] expected: Support,
) {
    let (library, shown) = library(amount, read_on, read, forge, differs);

    let assembled = library.assemble(&shown, &Caps::everything());

    assert_eq!(assembled.support.support, expected);
    assert_eq!(assembled.definition.read, expected != Support::Manual);
}

#[test]
fn an_approved_unmarked_draft_that_differs_names_the_clause_it_reads_wrong() {
    let (library, shown) = library(2, 2, ReadState::Approved, false, false);

    let record = library.support(&shown, &Caps::everything());

    assert_eq!(record.support, Support::Manual);
    assert!(
        record.clauses[0].reason.starts_with(READBACK_DIFFERS),
        "{record:?}"
    );
    assert_eq!(library.draft_state(&shown, &Caps::everything()), None);
}

#[test]
fn a_pending_differs_draft_is_to_read_in_a_deck_with_each_printed_clause_beside_the_engines_words()
{
    let (library, shown) = library(2, 2, ReadState::Pending, true, true);
    let facts = BTreeMap::from([(
        shown.oracle_id.clone(),
        CardFacts {
            printed: shown.clone(),
            color_identity: ColorSet::default(),
            legalities: BTreeMap::from([("freeform".to_owned(), Legality::Legal)]),
        },
    )]);
    let lines = [DeckLine {
        zone: DeckZone::Main,
        oracle_id: shown.oracle_id.clone(),
        count: 1,
    }];

    let report = deck_report("freeform", &lines, &facts, &library, &Caps::everything());

    assert_eq!(report.summary.to_read, 1);
    let card = &report.cards[0];
    assert_eq!(card.draft, Some(ReadState::Pending));
    assert_eq!(card.differs.len(), 1, "{card:?}");
    assert_eq!(
        card.differs[0].printed,
        "Lightning Bolt deals 3 damage to any target."
    );
    assert!(
        card.differs[0].written.contains('2'),
        "{:?}",
        card.differs[0]
    );
}

#[test]
fn a_draft_that_reads_back_has_no_differing_clauses_in_the_deck_report() {
    let (library, shown) = library(3, 3, ReadState::Pending, false, false);
    let facts = BTreeMap::from([(
        shown.oracle_id.clone(),
        CardFacts {
            printed: shown.clone(),
            color_identity: ColorSet::default(),
            legalities: BTreeMap::from([("freeform".to_owned(), Legality::Legal)]),
        },
    )]);
    let lines = [DeckLine {
        zone: DeckZone::Main,
        oracle_id: shown.oracle_id.clone(),
        count: 1,
    }];

    let report = deck_report("freeform", &lines, &facts, &library, &Caps::everything());

    assert_eq!(report.summary.to_read, 1);
    assert!(report.cards[0].differs.is_empty());
}
