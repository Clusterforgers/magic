//! Goal: a library answers a card's support, draft state and definition the same whether it decided the card before or not, and a library built again for a changed draft or verdict keeps what it took over only for the cards whose draft did not change.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::DraftRules;
use mtg_cards::ReadState;
use mtg_cards::load::Library;
use mtg_cards::support::{Caps, PrintedCard, read_hash};
use rstest::rstest;

use crate::common::{Corpus, printed};

const BOLT: &str = "Lightning Bolt";

/// The corpus, its printed cards and the Bolt's draft in `read`.
fn corpus_with_bolt(read: Option<ReadState>) -> (Library, Vec<PrintedCard>) {
    let base = Corpus::repo().library();
    let shown = printed();
    let Some(read) = read else {
        return (base, shown);
    };
    let card = base.card_by_name(BOLT).unwrap().clone();
    let bolt = shown.iter().find(|card| card.name == BOLT).unwrap();
    let row = DraftRules {
        forge: false,
        differs: false,
        proof: None,
        oracle_id: card.oracle_id.clone(),
        read_hash: read_hash(bolt, &card),
        rules: serde_json::to_value(&card).unwrap(),
        read,
    };
    let (library, left_out) = base.with_drafts(&[row]);
    assert!(left_out.is_empty(), "{left_out:?}");
    (library, shown)
}

/// Every answer of `library` on `cards` under `caps`.
fn answers(
    library: &Library,
    cards: &[PrintedCard],
    caps: &Caps,
) -> Vec<(String, String, Option<ReadState>, bool)> {
    cards
        .iter()
        .map(|card| {
            let support = library.support(card, caps);
            let assembled = library.assemble(card, caps);
            (
                support.fingerprint,
                assembled.support.fingerprint,
                library.draft_state(card, caps),
                assembled.definition.read,
            )
        })
        .collect()
}

#[rstest]
#[case(Caps::everything())]
#[case(Caps::none())]
fn a_card_decided_before_is_answered_as_a_fresh_library_answers_it(#[case] caps: Caps) {
    let (library, cards) = corpus_with_bolt(Some(ReadState::Pending));
    let fresh = answers(&library, &cards, &caps);
    assert_eq!(library.decided(), cards.len());
    assert_eq!(answers(&library, &cards, &caps), fresh);
    // Other capabilities are decided again, not taken from the first answers.
    let other = if caps == Caps::none() {
        Caps::everything()
    } else {
        Caps::none()
    };
    let (twin, _) = corpus_with_bolt(Some(ReadState::Pending));
    assert_eq!(
        answers(&library, &cards, &other),
        answers(&twin, &cards, &other)
    );
}

#[rstest]
#[case(None, Some(ReadState::Pending))]
#[case(Some(ReadState::Pending), Some(ReadState::Approved))]
#[case(Some(ReadState::Approved), Some(ReadState::Wrong))]
#[case(Some(ReadState::Wrong), Some(ReadState::Pending))]
#[case(Some(ReadState::Approved), None)]
fn a_library_built_again_for_a_verdict_takes_over_only_the_unchanged_cards(
    #[case] before: Option<ReadState>,
    #[case] after: Option<ReadState>,
) {
    let caps = Caps::everything();
    let (old, cards) = corpus_with_bolt(before);
    let _ = answers(&old, &cards, &caps);
    let (new, _) = corpus_with_bolt(after);
    new.inherit_decisions(&old);
    assert_eq!(
        new.decided(),
        cards.len() - 1,
        "all but the Bolt are taken over"
    );
    let (fresh, _) = corpus_with_bolt(after);
    assert_eq!(answers(&new, &cards, &caps), answers(&fresh, &cards, &caps));
}

#[test]
fn an_erratum_is_decided_again() {
    let caps = Caps::everything();
    let (library, cards) = corpus_with_bolt(Some(ReadState::Approved));
    let bolt = cards.iter().find(|card| card.name == BOLT).unwrap();
    let before = library.support(bolt, &caps);
    let mut changed = bolt.clone();
    changed.faces[0].oracle_text = "Lightning Bolt deals 2 damage to any target.".into();
    let (fresh, _) = corpus_with_bolt(Some(ReadState::Approved));
    assert_eq!(
        library.support(&changed, &caps),
        fresh.support(&changed, &caps)
    );
    assert_ne!(library.support(&changed, &caps), before);
}
