//! Goal: a trusted Forge draft is `Forge`, not `Read`: it is approved only when its read state is, runs like a read card (the definition is on a read, fail-soft resolution applies), has a fingerprint of its own, is counted apart in a deck whose state is still "runs on reads", and changes no `Verified` number.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{Corpus, printed};

use mtg_cards::card::Card;
use mtg_cards::deck::{CardFacts, DeckLine, DeckState, DeckZone, Legality, deck_report};
use mtg_cards::load::Library;
use mtg_cards::support::{Caps, PrintedCard, Support, read_hash};
use mtg_cards::{DraftRules, ReadState};
use mtg_core::color::ColorSet;
use rstest::rstest;

const BOLT: &str = "Lightning Bolt";

fn bolt() -> (Card, PrintedCard) {
    let card = Corpus::repo()
        .library()
        .card_by_name(BOLT)
        .expect("the corpus has Lightning Bolt")
        .clone();
    let shown = printed()
        .into_iter()
        .find(|card| card.name == BOLT)
        .expect("the corpus has Lightning Bolt");
    (card, shown)
}

fn library(read: ReadState, forge: bool) -> (Library, PrintedCard) {
    let (card, shown) = bolt();
    let row = DraftRules {
        oracle_id: card.oracle_id.clone(),
        read_hash: read_hash(&shown, &card),
        rules: serde_json::to_value(&card).unwrap(),
        read,
        forge,
        proof: None,
    };
    let (library, left_out) = Library::default().with_drafts(&[row]);
    assert!(left_out.is_empty(), "{left_out:?}");
    (library, shown)
}

#[rstest]
#[case::a_trusted_forge_draft_that_is_approved(ReadState::Approved, true, Support::Forge)]
#[case::a_player_approved_draft(ReadState::Approved, false, Support::Read)]
#[case::a_trusted_forge_draft_found_wrong(ReadState::Wrong, true, Support::Manual)]
#[case::a_trusted_forge_draft_still_pending(ReadState::Pending, true, Support::Manual)]
fn the_tier_of_a_draft_follows_its_source_and_its_read_state(
    #[case] read: ReadState,
    #[case] forge: bool,
    #[case] expected: Support,
) {
    let (library, shown) = library(read, forge);

    let assembled = library.assemble(&shown, &Caps::everything());

    assert_eq!(assembled.support.support, expected);
    assert_eq!(
        assembled.definition.read,
        expected != Support::Manual,
        "a card on a read is left to the table when its rules fail"
    );
    assert_eq!(
        assembled.support.support.runs_whole(),
        expected != Support::Manual
    );
}

#[test]
fn a_forge_card_and_a_read_card_have_different_fingerprints_and_neither_is_verified() {
    let (forge, shown) = library(ReadState::Approved, true);
    let (read, _) = library(ReadState::Approved, false);
    let caps = Caps::everything();

    let forge = forge.support(&shown, &caps);
    let read = read.support(&shown, &caps);

    assert_ne!(forge.fingerprint, read.fingerprint);
    assert!(forge.clauses.is_empty() && read.clauses.is_empty());
    assert_ne!(forge.support, Support::Verified);
}

#[test]
fn a_deck_of_verified_and_forge_cards_runs_on_reads_and_counts_the_copies_from_forge() {
    let (library, shown) = library(ReadState::Approved, true);
    let facts = std::collections::BTreeMap::from([(
        shown.oracle_id.clone(),
        CardFacts {
            printed: shown.clone(),
            color_identity: ColorSet::default(),
            legalities: std::collections::BTreeMap::from([(
                "freeform".to_owned(),
                Legality::Legal,
            )]),
        },
    )]);
    let lines = [DeckLine {
        zone: DeckZone::Main,
        oracle_id: shown.oracle_id.clone(),
        count: 4,
    }];

    let report = deck_report("freeform", &lines, &facts, &library, &Caps::everything());

    assert_eq!(report.summary.counts.forge, 4);
    assert_eq!(report.summary.counts.read, 0);
    assert_eq!(report.summary.counts.verified, 0);
    assert_eq!(report.summary.state, DeckState::Read);
}
