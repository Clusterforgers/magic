//! Goal: the fix-mark bodies keep their documented wire shape: a note is optional and unknown members are refused, a list survives a JSON round trip, the note limit is the database's, and the TypeScript bindings carry all of it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::generated;
use mtg_protocol::rest::{FixMarkBody, FixMarkList, FixMarkView, MAX_FIX_NOTE_CHARS};
use rstest::rstest;
use serde_json::json;

#[rstest]
#[case(json!({}), None)]
#[case(json!({"note": null}), None)]
#[case(json!({"note": "does not draw"}), Some("does not draw"))]
fn a_note_is_optional(#[case] body: serde_json::Value, #[case] note: Option<&str>) {
    let read: FixMarkBody = serde_json::from_value(body).unwrap();
    assert_eq!(read.note.as_deref(), note);
}

#[rstest]
#[case(json!({"notes": "typo"}))]
#[case(json!({"note": 3}))]
#[case(json!({"note": "x", "extra": true}))]
fn a_body_with_other_members_or_kinds_is_refused(#[case] body: serde_json::Value) {
    assert!(serde_json::from_value::<FixMarkBody>(body).is_err());
}

#[test]
fn a_list_survives_a_json_round_trip_with_the_documented_keys() {
    let list = FixMarkList {
        marks: vec![
            FixMarkView {
                oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".to_owned(),
                name: "Sol Ring".to_owned(),
                note: Some("adds the wrong mana".to_owned()),
                created_at: "2026-10-29T12:00:00Z".to_owned(),
            },
            FixMarkView {
                oracle_id: "4457ed35-7c10-48c8-9776-456485fdf071".to_owned(),
                name: "Opt".to_owned(),
                note: None,
                created_at: "2026-10-28T12:00:00Z".to_owned(),
            },
        ],
    };
    let json = serde_json::to_value(&list).unwrap();
    assert_eq!(json["marks"][1]["note"], json!(null));
    let mut keys: Vec<&str> = json["marks"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["created_at", "name", "note", "oracle_id"]);
    assert_eq!(serde_json::from_value::<FixMarkList>(json).unwrap(), list);
}

#[test]
fn the_note_limit_is_the_one_the_database_checks() {
    assert_eq!(MAX_FIX_NOTE_CHARS, 280);
}

#[test]
fn the_bindings_carry_the_fix_mark_types() {
    let files = generated();
    for needed in ["FixMarkBody.ts", "FixMarkList.ts", "FixMarkView.ts"] {
        assert!(files.contains_key(needed), "missing {needed}");
        assert!(files["index.ts"].contains(needed.trim_end_matches(".ts")));
    }
    assert!(files["FixMarkBody.ts"].contains("note: string | null"));
    assert!(files["FixMarkView.ts"].contains("note: string | null"));
    assert!(files["FixMarkList.ts"].contains("marks: Array<FixMarkView>"));
}
