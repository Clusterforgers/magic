//! Goal: rules whose clause reads back as other text than the card (`Differs`) never play automatically, whatever their source: a compiled row or a draft plays by hand with the reason `readback:Differs`, rules that read back or cannot be read back (`Unchecked`) keep their state, and no `Verified` corpus card reads `Differs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};

use mtg_cards::card::Card;
use mtg_cards::load::{GeneratedRules, Library};
use mtg_cards::proposal::{ClauseState, Verdict, check_exact, differing};
use mtg_cards::support::{
    Caps, PrintedCard, READBACK_DIFFERS, Support, SupportRecord, read_hash, support,
};
use mtg_cards::{DraftRules, ReadState};

/// Corpus cards whose `Differs` a reviewed corpus test overrides: the card plays right and the writer lacks the wording.
const REVIEWED: &[&str] = &[];

fn explore() -> PrintedCard {
    common::printed()
        .into_iter()
        .find(|card| card.name == "Explore")
        .unwrap()
}

/// Explore's rules with this draw effect after its extra land play.
fn rules(printed: &PrintedCard, draw: &Value) -> Card {
    serde_json::from_value(json!({
        "oracle_id": printed.oracle_id,
        "name": "Explore",
        "impl_status": "Full",
        "faces": [{
            "name": "Explore",
            "oracle_fingerprint": mtg_cards::support::oracle_fingerprint(&printed.faces[0].oracle_text),
            "abilities": [{"Spell": {"effects": [
                {"Rule": {"rule": {"AdditionalLandPlays": {}}, "duration": "UntilEndOfTurn"}},
                draw,
            ]}}],
        }],
    }))
    .unwrap()
}

fn generated(printed: &PrintedCard, card: &Card) -> SupportRecord {
    let row = GeneratedRules {
        oracle_id: printed.oracle_id.clone(),
        rules: serde_json::to_value(card).unwrap(),
    };
    let (library, left_out) = Library::default().with_generated(&[row]);
    assert!(left_out.is_empty(), "{left_out:?}");
    library.support(printed, &Caps::everything())
}

fn drafted(printed: &PrintedCard, card: &Card) -> SupportRecord {
    let row = DraftRules {
        oracle_id: printed.oracle_id.clone(),
        read_hash: read_hash(printed, card),
        rules: serde_json::to_value(card).unwrap(),
        read: ReadState::Approved,
        forge: true,
        proof: None,
    };
    let (library, left_out) = Library::default().with_drafts(&[row]);
    assert!(left_out.is_empty(), "{left_out:?}");
    library.support(printed, &Caps::everything())
}

#[rstest]
#[case::reads(json!({"Draw": {}}), Verdict::Accepted, Support::Verified, Support::Forge)]
#[case::another_count(json!({"Draw": {"count": 2}}), Verdict::Mismatch, Support::Manual, Support::Manual)]
#[case::another_player(
    json!({"Draw": {"who": "EachPlayer"}}),
    Verdict::Mismatch,
    Support::Manual,
    Support::Manual
)]
#[case::unwritten(
    json!({"Draw": {"count": {"Count": [{"Named": "Explore"}, {"In": "Graveyard"}]}}}),
    Verdict::Unchecked,
    Support::Verified,
    Support::Forge
)]
fn rules_that_read_back_as_other_text_play_by_hand_with_the_reason(
    #[case] draw: Value,
    #[case] verdict: Verdict,
    #[case] compiled: Support,
    #[case] draft: Support,
) {
    let printed = explore();
    let card = rules(&printed, &draw);
    let text = serde_json::to_string(&card).unwrap();
    let report = check_exact(
        &text,
        &printed,
        &Library::default(),
        &BTreeSet::from(["explore".to_owned()]),
        &Caps::everything(),
    );
    assert_eq!(report.verdict(), verdict, "{:?}", report.clauses);
    let differs = verdict == Verdict::Mismatch;
    assert_eq!(!differing(&card, &printed).is_empty(), differs);
    for (record, expected) in [
        (generated(&printed, &card), compiled),
        (drafted(&printed, &card), draft),
    ] {
        assert_eq!(record.support, expected, "{record:?}");
        let named = record
            .clauses
            .iter()
            .any(|clause| clause.reason.starts_with(READBACK_DIFFERS));
        assert_eq!(named, differs, "{record:?}");
    }
}

/// No `Verified` corpus card has a clause that reads back as other text, unless a reviewed test overrides it.
#[test]
fn no_verified_corpus_card_reads_back_as_other_text() {
    let library = common::Corpus::repo().library();
    let caps = Caps::everything();
    let mut found = Vec::new();
    for printed in common::printed() {
        let Some(card) = library.card(&printed.oracle_id) else {
            continue;
        };
        if REVIEWED.contains(&printed.name.as_str())
            || support(Some(card), &printed, &caps).support != Support::Verified
        {
            continue;
        }
        for clause in differing(card, &printed) {
            if let ClauseState::Differs { written, .. } = &clause.state {
                found.push(format!("{}: {} / {written}", printed.name, clause.text));
            }
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}
