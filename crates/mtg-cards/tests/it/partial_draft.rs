//! Goal: a proved partial draft plays in state `Partial` on a read (fail-soft, pinned) only when it is approved, its text and engine fit, and it keeps strictly less by hand than the compiled `Partial` row; a `wrong` verdict or an edit turns the card back to the row.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::card::{Card, Face, ImplStatus, Span};
use mtg_cards::effect::Effect;
use mtg_cards::load::{GeneratedRules, Library};
use mtg_cards::proposal::partial::{ClauseMark, Mark, PartialProof};
use mtg_cards::refs::{Bounds, Ref, TargetSpec, Targetable};
use mtg_cards::support::{Caps, PrintedCard, PrintedFace, Support, oracle_fingerprint, read_hash};
use mtg_cards::value::Amount;
use mtg_cards::{DraftRules, ReadState};
use rstest::rstest;

const ID: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
const LINES: [&str; 3] = [
    "Lightning Bolt deals 3 damage to any target.",
    "Whenever you cast a spell, scry 1.",
    "Creatures you control get +1/+1.",
];

fn text() -> String {
    LINES.join("\n")
}

fn span(line: usize) -> Span {
    let start: usize = LINES[..line].iter().map(|l| l.chars().count() + 1).sum();
    Span {
        start: u32::try_from(start).unwrap(),
        end: u32::try_from(start + LINES[line].chars().count()).unwrap(),
    }
}

fn printed(oracle: &str) -> PrintedCard {
    PrintedCard {
        layout: String::new(),
        oracle_id: ID.into(),
        name: "Lightning Bolt".into(),
        keywords: vec![],
        faces: vec![PrintedFace {
            name: "Lightning Bolt".into(),
            oracle_text: oracle.into(),
            ..PrintedFace::default()
        }],
    }
}

fn bolt() -> Ability {
    Ability::Spell {
        targets: vec![TargetSpec {
            what: Targetable::AnyTarget,
            bounds: Bounds::default(),
            divide: None,
        }],
        effects: vec![Effect::DealDamage {
            source: None,
            to: Ref::Target(0),
            amount: Amount::Fixed(3),
        }],
    }
}

/// A partial card whose lines in `by_hand` (indices into `LINES`) are `Manual`; line 0 always runs.
fn card(by_hand: &[usize]) -> Card {
    let mut abilities = vec![bolt()];
    abilities.extend(by_hand.iter().map(|&line| Ability::Manual {
        reason: "by hand".into(),
        span: span(line),
    }));
    Card {
        oracle_id: ID.into(),
        name: "Lightning Bolt".into(),
        impl_status: ImplStatus::Partial,
        faces: vec![Face {
            name: "Lightning Bolt".into(),
            oracle_fingerprint: oracle_fingerprint(&text()),
            abilities,
        }],
        notes: None,
    }
}

fn proof(by_hand: &[usize]) -> PartialProof {
    PartialProof {
        marks: (0..LINES.len())
            .map(|line| ClauseMark {
                face: 0,
                span: span(line),
                mark: if by_hand.contains(&line) {
                    Mark::ByHand
                } else {
                    Mark::Auto
                },
            })
            .collect(),
    }
}

fn draft(by_hand: &[usize], read: ReadState) -> DraftRules {
    let card = card(by_hand);
    DraftRules {
        oracle_id: ID.into(),
        read_hash: read_hash(&printed(&text()), &card),
        rules: serde_json::to_value(&card).unwrap(),
        read,
        forge: true,
        proof: Some(proof(by_hand)),
    }
}

fn row(by_hand: &[usize]) -> Library {
    let generated = GeneratedRules {
        oracle_id: ID.into(),
        rules: serde_json::to_value(card(by_hand)).unwrap(),
    };
    let (library, left_out) = Library::default().with_generated(&[generated]);
    assert!(left_out.is_empty(), "{left_out:?}");
    library
}

/// The state, the clauses left by hand and whether the card plays on a draft (fails soft, pins).
fn played(library: &Library, shown: &PrintedCard, caps: &Caps) -> (Support, usize, bool) {
    let assembled = library.assemble(shown, caps);
    (
        assembled.support.support,
        assembled.definition.manual.len(),
        assembled.definition.read,
    )
}

#[rstest]
#[case::fewer_by_hand_beats_the_row(&[1, 2], &[1], true)]
#[case::the_same_lines_keep_the_row(&[1], &[1], false)]
#[case::more_by_hand_keeps_the_row(&[1], &[1, 2], false)]
#[case::other_lines_keep_the_row(&[1], &[2], false)]
fn a_draft_beats_a_compiled_partial_row_only_when_it_keeps_strictly_less_by_hand(
    #[case] compiled: &[usize],
    #[case] drafted: &[usize],
    #[case] draft_plays: bool,
) {
    let (library, left_out) = row(compiled).with_drafts(&[draft(drafted, ReadState::Approved)]);
    assert!(left_out.is_empty(), "{left_out:?}");

    let (state, by_hand, read) = played(&library, &printed(&text()), &Caps::everything());

    let expected = if draft_plays { drafted } else { compiled };
    assert_eq!(state, Support::Partial);
    assert_eq!(by_hand, expected.len());
    assert_eq!(read, draft_plays, "only a draft fails soft and pins");
}

#[rstest]
#[case::approved(ReadState::Approved, true)]
#[case::found_wrong(ReadState::Wrong, false)]
#[case::pending(ReadState::Pending, false)]
fn a_wrong_verdict_turns_the_card_back_to_its_compiled_row(
    #[case] read: ReadState,
    #[case] draft_plays: bool,
) {
    let (library, _) = row(&[1, 2]).with_drafts(&[draft(&[1], read)]);

    let (state, by_hand, plays) = played(&library, &printed(&text()), &Caps::everything());

    assert_eq!(state, Support::Partial);
    assert_eq!(by_hand, if draft_plays { 1 } else { 2 });
    assert_eq!(plays, draft_plays);
}

#[test]
fn a_card_with_no_compiled_row_plays_on_the_draft_and_is_never_whole() {
    let (library, left_out) =
        Library::default().with_drafts(&[draft(&[1, 2], ReadState::Approved)]);
    assert!(left_out.is_empty(), "{left_out:?}");

    let (state, by_hand, read) = played(&library, &printed(&text()), &Caps::everything());

    assert_eq!((state, by_hand, read), (Support::Partial, 2, true));
    assert!(!state.runs_whole());
}

#[test]
fn an_edit_of_the_printed_text_turns_the_card_back() {
    let (library, _) = row(&[1, 2]).with_drafts(&[draft(&[1], ReadState::Approved)]);

    let errata = printed(&format!("{}\nAnd more.", text()));
    let (_, _, read) = played(&library, &errata, &Caps::everything());

    assert!(!read, "a verdict on another hash counts for nothing");
}

#[test]
fn a_draft_the_engine_cannot_run_does_not_play() {
    let (library, _) = Library::default().with_drafts(&[draft(&[1], ReadState::Approved)]);

    let (state, _, read) = played(&library, &printed(&text()), &Caps::none());

    assert_ne!(state, Support::Partial);
    assert!(!read, "no line turns by hand behind the proof");
}

#[rstest]
#[case::no_proof(None, true)]
#[case::not_trusted(Some(proof(&[1])), false)]
#[case::other_lines(Some(proof(&[2])), true)]
fn a_partial_draft_without_a_matching_proof_is_left_out(
    #[case] proved: Option<PartialProof>,
    #[case] forge: bool,
) {
    let mut row = draft(&[1], ReadState::Approved);
    row.proof = proved;
    row.forge = forge;

    let (library, left_out) = Library::default().with_drafts(&[row]);

    assert_eq!(left_out.len(), 1, "{left_out:?}");
    assert!(library.draft(ID).is_none());
}

#[test]
fn a_whole_draft_still_needs_no_proof() {
    let mut whole = draft(&[], ReadState::Approved);
    whole.rules = serde_json::to_value(Card {
        impl_status: ImplStatus::Full,
        ..card(&[])
    })
    .unwrap();
    whole.proof = None;

    let (library, left_out) = Library::default().with_drafts(&[whole]);

    assert!(left_out.is_empty(), "{left_out:?}");
    assert!(library.draft(ID).is_some_and(|d| d.proof.is_none()));
}
