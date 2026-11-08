//! Goal: the read bodies keep their documented wire shape: a verdict is a `snake_case` word, the note is optional and unknown members are refused, a card with its sheet survives a JSON round trip, and the TypeScript bindings carry all of it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::generated;
use mtg_cards::ReadState;
use mtg_cards::card::Span;
use mtg_cards::compile::ClauseKind;
use mtg_cards::read::{ReadRow, ReadSheet, RowState, Step};
use mtg_protocol::rest::{MyRead, ReadBody, ReadCard, ReadQueue, ReadQueueCard, ReadVerdict};
use rstest::rstest;
use serde_json::json;

#[rstest]
#[case(json!({"read_hash": "ab", "verdict": "correct"}), ReadVerdict::Correct, None)]
#[case(json!({"read_hash": "ab", "verdict": "wrong", "note": "draws two"}), ReadVerdict::Wrong, Some("draws two"))]
#[case(json!({"read_hash": "ab", "verdict": "wrong", "note": null}), ReadVerdict::Wrong, None)]
fn a_body_names_the_hash_and_the_verdict(
    #[case] body: serde_json::Value,
    #[case] verdict: ReadVerdict,
    #[case] note: Option<&str>,
) {
    let read: ReadBody = serde_json::from_value(body).unwrap();
    assert_eq!(read.read_hash, "ab");
    assert_eq!(read.verdict, verdict);
    assert_eq!(read.note.as_deref(), note);
}

#[rstest]
#[case(json!({"verdict": "correct"}))]
#[case(json!({"read_hash": "ab", "verdict": "Correct"}))]
#[case(json!({"read_hash": "ab", "verdict": "maybe"}))]
#[case(json!({"read_hash": "ab", "verdict": "wrong", "extra": 1}))]
fn a_body_without_a_hash_or_with_other_words_is_refused(#[case] body: serde_json::Value) {
    assert!(serde_json::from_value::<ReadBody>(body).is_err());
}

fn card() -> ReadCard {
    ReadCard {
        oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
        name: "Opt".into(),
        read_hash: "0123456789abcdef0123456789abcdef".into(),
        state: ReadState::Pending,
        correct: 0,
        wrong: 0,
        forge: false,
        sheet: ReadSheet {
            card: "Opt".into(),
            faces: vec!["Opt".into()],
            rows: vec![ReadRow {
                face: 0,
                span: Span { start: 0, end: 7 },
                kind: ClauseKind::Spell,
                printed: "Scry 1.".into(),
                state: RowState::Differs {
                    written: "Surveil 1.".into(),
                },
                abilities: vec![0],
                continues: false,
                label: "spell".into(),
                wording: Some("Surveil 1.".into()),
                steps: vec![Step {
                    depth: 0,
                    text: "surveil 1".into(),
                }],
            }],
            extra: vec![],
        },
        mine: Some(MyRead {
            verdict: ReadVerdict::Wrong,
            note: Some("surveils".into()),
            created_at: "2026-11-08T12:00:00Z".into(),
            current: true,
        }),
    }
}

#[test]
fn a_card_survives_a_json_round_trip_with_its_sheet() {
    let card = card();
    let value = serde_json::to_value(&card).unwrap();
    assert_eq!(value["state"], json!("pending"));
    assert_eq!(value["mine"]["verdict"], json!("wrong"));
    assert_eq!(
        value["sheet"]["rows"][0]["state"],
        json!({"differs": {"written": "Surveil 1."}})
    );
    assert_eq!(serde_json::from_value::<ReadCard>(value).unwrap(), card);
}

#[test]
fn a_queue_lists_cards_with_their_state() {
    let queue = ReadQueue {
        title: "Deck".into(),
        cards: vec![ReadQueueCard {
            oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
            name: "Opt".into(),
            read_hash: "ab".into(),
            state: ReadState::Approved,
            differs: 2,
            rank: Some(12),
            in_decks: true,
        }],
    };
    let value = serde_json::to_value(&queue).unwrap();
    assert_eq!(value["cards"][0]["state"], json!("approved"));
    assert_eq!(value["cards"][0]["rank"], json!(12));
    assert_eq!(serde_json::from_value::<ReadQueue>(value).unwrap(), queue);
}

#[test]
fn the_bindings_carry_the_read_types() {
    let files = generated();
    for needed in [
        "ReadBody.ts",
        "ReadCard.ts",
        "ReadQueue.ts",
        "ReadSheet.ts",
        "ReadRow.ts",
        "RowState.ts",
        "CardDraft.ts",
    ] {
        assert!(files.contains_key(needed), "missing {needed}");
    }
    assert!(files["ReadVerdict.ts"].contains("\"correct\" | \"wrong\""));
    assert!(files["CardDetail.ts"].contains("draft?: CardDraft"));
}
