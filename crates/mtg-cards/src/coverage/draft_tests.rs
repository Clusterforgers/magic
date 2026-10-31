//! Goal: a draft counts by its read when no rules file or `Verified` reading outranks it (`Read` once approved, waiting while pending or stale, by hand when wrong or when the engine lacks a primitive), and the tally counts `Read` and the waiting cards apart from `Partial` and `Manual`.
use super::*;
use crate::ability::Ability;
use crate::card::{Face, Span};
use crate::compile::compile;
use crate::draft::{DraftRules, ReadState};
use crate::load::GeneratedRules;
use crate::support::{PrintedFace, oracle_fingerprint, read_hash};
use rstest::rstest;

fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}

fn sorcery(text: &str) -> PrintedCard {
    PrintedCard {
        layout: String::new(),
        oracle_id: id(7),
        name: "Test Card".into(),
        keywords: vec![],
        faces: vec![PrintedFace {
            name: "Test Card".into(),
            type_line: "Sorcery".into(),
            oracle_text: text.into(),
            ..PrintedFace::default()
        }],
    }
}

/// What the engine of these tests runs: spells that draw.
fn caps() -> Caps {
    Caps::from_names(["Ability.Spell", "Effect.Draw"])
}

/// The rules file of card 7 as the compiler reads `shown`, declared `status`.
fn filed(shown: &PrintedCard, status: ImplStatus) -> Card {
    let mut card = compile(shown).card.unwrap_or_else(|| Card {
        oracle_id: id(7),
        name: "Test Card".into(),
        impl_status: status,
        faces: vec![Face {
            name: "Test Card".into(),
            oracle_fingerprint: oracle_fingerprint(&shown.faces[0].oracle_text),
            abilities: vec![],
        }],
        notes: None,
    });
    card.impl_status = status;
    card
}

/// A draft of card 7 that draws a card, written for `shown`, read as `read` on `hash` (the current one when `None`).
fn draft_of(shown: &PrintedCard, read: ReadState, hash: Option<&str>) -> DraftRules {
    let draw = sorcery("Draw a card.");
    let mut card = filed(&draw, ImplStatus::Full);
    card.faces[0].oracle_fingerprint = oracle_fingerprint(&shown.faces[0].oracle_text);
    DraftRules {
        oracle_id: id(7),
        read_hash: hash.map_or_else(|| read_hash(shown, &card), str::to_owned),
        rules: serde_json::to_value(&card).unwrap_or_default(),
        read,
    }
}

fn judged(shown: &PrintedCard, library: &Library, caps: &Caps) -> CardResult {
    let entry = PreconCard {
        printed: shown.clone(),
        copies: BTreeMap::from([("deck".to_owned(), 1)]),
    };
    let report = analyze_with(&[entry], library, caps);
    assert_eq!(report.cards.len(), 1);
    report
        .cards
        .into_iter()
        .next()
        .unwrap_or_else(|| unreachable!())
}

fn with_draft(library: &Library, draft: DraftRules) -> Library {
    let (library, left_out) = library.with_drafts(&[draft]);
    assert!(left_out.is_empty(), "{left_out:?}");
    library
}

#[rstest]
#[case::pending_waits(ReadState::Pending, None, "Wish for a card.", &[], Support::Manual, true)]
#[case::approved_reads(ReadState::Approved, None, "Wish for a card.", &[], Support::Read, false)]
#[case::wrong_stays_by_hand(ReadState::Wrong, None, "Wish for a card.", &[], Support::Manual, false)]
#[case::approved_on_an_old_hash_waits_again(
    ReadState::Approved,
    Some("0123"),
    "Wish for a card.",
    &[],
    Support::Manual,
    true
)]
#[case::approved_but_the_engine_lacks_a_primitive(
    ReadState::Approved,
    None,
    "Wish for a card.",
    &["Ability.Spell"],
    Support::Manual,
    false
)]
#[case::a_verified_reading_outranks_the_draft(
    ReadState::Pending,
    None,
    "Draw a card.",
    &[],
    Support::Verified,
    false
)]
fn a_draft_counts_by_its_read_when_nothing_outranks_it(
    #[case] read: ReadState,
    #[case] hash: Option<&str>,
    #[case] text: &str,
    #[case] engine: &[&str],
    #[case] state: Support,
    #[case] waiting: bool,
) {
    let shown = sorcery(text);
    let caps = if engine.is_empty() {
        caps()
    } else {
        Caps::from_names(engine.iter().copied())
    };
    let library = with_draft(&Library::default(), draft_of(&shown, read, hash));

    let result = judged(&shown, &library, &caps);

    assert_eq!((result.support, result.waiting), (state, waiting));
    let bucket = match (state, waiting) {
        (_, true) => Bucket::Waiting,
        (Support::Read, _) => Bucket::Read,
        (Support::Verified, _) => Bucket::Verified,
        _ => Bucket::Manual,
    };
    assert_eq!(result.bucket(), bucket);
    // Read and Verified cards have nothing holding them back; the others keep their blockers.
    assert_eq!(
        result.blockers.is_empty(),
        matches!(state, Support::Read | Support::Verified)
    );
    let source = if state == Support::Read {
        RulesSource::Draft
    } else {
        RulesSource::Compiled
    };
    assert_eq!(result.rules, source);
}

#[rstest]
#[case::pending(ReadState::Pending)]
#[case::approved(ReadState::Approved)]
fn a_rules_file_outranks_a_draft(#[case] read: ReadState) {
    let shown = sorcery("Wish for a card.");
    let file = Library::from_sources(&[Source {
        path: "cards/test_card.json".into(),
        text: canonical(&filed(&shown, ImplStatus::Manual)),
    }])
    .unwrap_or_else(|errors| panic!("{errors}"));
    let library = with_draft(&file, draft_of(&shown, read, None));

    let result = judged(&shown, &library, &caps());

    assert_eq!((result.support, result.waiting), (Support::Manual, false));
    assert_eq!(result.rules, RulesSource::HandWritten);
}

#[rstest]
#[case::pending(ReadState::Pending, Support::Partial, true)]
#[case::approved(ReadState::Approved, Support::Read, false)]
fn a_compiled_row_that_is_not_verified_gives_way_to_a_draft(
    #[case] read: ReadState,
    #[case] support: Support,
    #[case] waiting: bool,
) {
    // A row that draws a card and leaves a clause to the table.
    let shown = sorcery("Draw a card.\nWish for a card.");
    let mut partial = draft_of(&shown, ReadState::Pending, None);
    let mut card: Card =
        serde_json::from_value(partial.rules.clone()).unwrap_or_else(|e| panic!("{e}"));
    card.impl_status = ImplStatus::Partial;
    card.faces[0].abilities.push(Ability::Manual {
        reason: "left to the table".into(),
        span: Span { start: 14, end: 31 },
    });
    partial.rules = serde_json::to_value(&card).unwrap_or_default();
    let row = GeneratedRules {
        oracle_id: id(7),
        rules: partial.rules,
    };
    let (rows, left_out) = Library::default().with_generated(&[row]);
    assert!(left_out.is_empty(), "{left_out:?}");
    let library = with_draft(&rows, draft_of(&shown, read, None));

    let result = judged(&shown, &library, &caps());

    assert_eq!((result.support, result.waiting), (support, waiting));
}

#[test]
fn the_tally_counts_read_and_waiting_cards_apart_from_partial_and_manual() {
    let card = |name: &str, support: Support, waiting: bool, copies: u32| CardResult {
        oracle_id: name.into(),
        name: name.into(),
        rules: RulesSource::Compiled,
        support,
        blockers: BTreeSet::new(),
        waiting,
        copies: BTreeMap::from([("deck".to_owned(), copies)]),
    };
    let report = Report {
        cards: vec![
            card("a", Support::Verified, false, 1),
            card("b", Support::Read, false, 2),
            card("c", Support::Manual, true, 4),
            card("d", Support::Partial, true, 8),
            card("e", Support::Partial, false, 16),
            card("f", Support::Stale, false, 32),
        ],
    };

    let tally: Vec<_> = report
        .tally()
        .into_iter()
        .map(|row| (row.bucket.name(), row.cards, row.copies))
        .collect();

    assert_eq!(
        tally,
        [
            ("verified", 1, 1),
            ("read", 1, 2),
            ("waiting", 2, 12),
            ("partial", 1, 16),
            ("manual", 0, 0),
            ("other", 1, 32),
        ]
    );
    let shares: Vec<_> = report
        .decks()
        .into_iter()
        .map(|deck| (deck.verified, deck.copies))
        .collect();
    // The share of a deck counts proven cards only.
    assert_eq!(shares, [(1, 63)]);
}
