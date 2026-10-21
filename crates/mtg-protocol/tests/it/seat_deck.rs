//! Goal: the deck a seat plays keeps its documented wire shape: the roster lists a deck's name, format and manual-card count and leaves the field out for a seat without one, any table of decks survives a JSON round trip, the bodies that name a deck refuse anything else, the refusal of a blocked deck lists its issues, every deck error code has one spelling, and the TypeScript bindings carry all of it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::deck::{DeckZone, Issue, IssueCode};
use mtg_core::ids::PlayerId;
use mtg_protocol::bindings::export_to;
use mtg_protocol::rest::{
    ChooseDeck, DeckBlockedBody, DeckChoiceErrorCode, JoinCodeRequest, JoinRequest,
};
use mtg_protocol::{LobbySeat, SeatDeck, ServerMessage};
use proptest::prelude::*;
use rstest::rstest;
use serde_json::{Value, json};

fn deck(name: &str, format: &str, manual_cards: u32) -> SeatDeck {
    SeatDeck {
        name: name.to_owned(),
        format: format.to_owned(),
        manual_cards,
    }
}

fn seat(index: u64, deck: Option<SeatDeck>) -> LobbySeat {
    LobbySeat {
        seat: PlayerId(index),
        name: format!("Seat {index}"),
        connected: true,
        ready: false,
        deck,
    }
}

#[test]
fn a_seat_without_a_deck_leaves_the_field_out_and_a_seat_with_one_names_it() {
    let roster = ServerMessage::Lobby {
        seats: vec![seat(0, Some(deck("Burn", "modern", 3))), seat(1, None)],
        needed: 2,
    };
    assert_eq!(
        serde_json::to_value(&roster).unwrap(),
        json!({
            "type": "Lobby",
            "seats": [
                {
                    "seat": 0, "name": "Seat 0", "connected": true, "ready": false,
                    "deck": {"name": "Burn", "format": "modern", "manual_cards": 3},
                },
                {"seat": 1, "name": "Seat 1", "connected": true, "ready": false},
            ],
            "needed": 2,
        })
    );
}

#[test]
fn a_roster_from_a_server_that_predates_decks_still_decodes() {
    let old = json!({
        "type": "Lobby",
        "seats": [{"seat": 0, "name": "Ann", "connected": true, "ready": false}],
        "needed": 1,
    });
    let ServerMessage::Lobby { seats, .. } = serde_json::from_value(old).unwrap() else {
        panic!("not a roster");
    };
    assert_eq!(seats[0].deck, None);
}

#[rstest]
#[case::missing_count(json!({"name": "Burn", "format": "modern"}))]
#[case::negative_count(json!({"name": "Burn", "format": "modern", "manual_cards": -1}))]
#[case::count_as_text(json!({"name": "Burn", "format": "modern", "manual_cards": "3"}))]
#[case::name_as_number(json!({"name": 7, "format": "modern", "manual_cards": 0}))]
#[case::not_an_object(json!("Burn"))]
fn a_deck_that_is_not_a_name_a_format_and_a_count_is_refused(#[case] body: Value) {
    assert!(serde_json::from_value::<SeatDeck>(body.clone()).is_err());
    let roster = json!({
        "type": "Lobby",
        "seats": [{"seat": 0, "name": "A", "connected": true, "ready": true, "deck": body}],
        "needed": 0,
    });
    assert!(serde_json::from_value::<ServerMessage>(roster).is_err());
}

fn text() -> impl Strategy<Value = String> {
    prop_oneof!["[A-Za-z0-9 _-]{1,40}", "\\PC{1,40}"]
}

fn table() -> impl Strategy<Value = Vec<LobbySeat>> {
    (2usize..=12).prop_flat_map(|seats| {
        prop::collection::vec(prop::option::of((text(), text(), 0u32..=250)), seats).prop_map(
            |rows| {
                rows.into_iter()
                    .enumerate()
                    .map(|(index, row)| {
                        let deck = row.map(|(name, format, count)| deck(&name, &format, count));
                        seat(u64::try_from(index).unwrap(), deck)
                    })
                    .collect()
            },
        )
    })
}

proptest! {
    #[test]
    fn any_table_of_decks_round_trips_and_names_no_card(seats in table()) {
        let message = ServerMessage::Lobby { seats, needed: 0 };
        let text = serde_json::to_string(&message).unwrap();
        let back: ServerMessage = serde_json::from_str(&text).unwrap();
        prop_assert_eq!(&back, &message);
        let value: Value = serde_json::from_str(&text).unwrap();
        for row in value["seats"].as_array().unwrap() {
            let mut fields: Vec<&str> = row.as_object().unwrap().keys().map(String::as_str).collect();
            fields.sort_unstable();
            if let Some(deck) = row.get("deck") {
                // The table learns three facts of a deck and nothing that could be a card.
                let mut facts: Vec<&str> = deck.as_object().unwrap().keys().map(String::as_str).collect();
                facts.sort_unstable();
                prop_assert_eq!(facts, ["format", "manual_cards", "name"]);
                prop_assert_eq!(fields, ["connected", "deck", "name", "ready", "seat"]);
            } else {
                prop_assert_eq!(fields, ["connected", "name", "ready", "seat"]);
            }
        }
    }
}

#[test]
fn the_bodies_that_name_a_deck_leave_it_out_when_there_is_none_and_refuse_other_fields() {
    let bare = JoinRequest::default();
    assert_eq!(serde_json::to_value(&bare).unwrap(), json!({}));
    let named = JoinRequest {
        name: Some("Bo".into()),
        deck: Some("mono-red".into()),
    };
    assert_eq!(
        serde_json::to_value(&named).unwrap(),
        json!({"name": "Bo", "deck": "mono-red"})
    );
    let by_code = JoinCodeRequest {
        code: "ABCD2345".into(),
        name: None,
        deck: Some("4457ed35-7c10-48c8-9776-456485fdf070".into()),
    };
    assert_eq!(
        serde_json::to_value(&by_code).unwrap(),
        json!({"code": "ABCD2345", "deck": "4457ed35-7c10-48c8-9776-456485fdf070"})
    );
    assert_eq!(
        serde_json::from_value::<JoinRequest>(json!({"deck": "mono-red"}))
            .unwrap()
            .deck
            .as_deref(),
        Some("mono-red")
    );
    assert!(serde_json::from_value::<JoinRequest>(json!({"decks": "mono-red"})).is_err());
    assert!(serde_json::from_value::<JoinRequest>(json!({"deck": 7})).is_err());
}

#[rstest]
#[case::the_deck(json!({"deck": "mono-green"}), true)]
#[case::an_id(json!({"deck": "4457ed35-7c10-48c8-9776-456485fdf070"}), true)]
#[case::missing(json!({}), false)]
#[case::null(json!({"deck": null}), false)]
#[case::a_number(json!({"deck": 3}), false)]
#[case::an_extra_field(json!({"deck": "mono-green", "seat": 1}), false)]
#[case::an_object(json!({"deck": {"kind": "demo", "id": "mono-green"}}), false)]
fn the_body_of_a_put_names_one_deck_and_nothing_else(#[case] body: Value, #[case] fine: bool) {
    assert_eq!(serde_json::from_value::<ChooseDeck>(body).is_ok(), fine);
}

fn issue() -> Issue {
    Issue {
        code: IssueCode::Copies,
        message: "Lightning Bolt appears 5 times; at most 4 are allowed".into(),
        zone: Some(DeckZone::Main),
        oracle_id: Some("4457ed35-7c10-48c8-9776-456485fdf070".into()),
        name: Some("Lightning Bolt".into()),
    }
}

#[test]
fn a_refused_deck_lists_its_issues_under_the_code_of_the_refusal() {
    let body = DeckBlockedBody {
        error: DeckChoiceErrorCode::DeckBlocked,
        message: "the deck cannot be played".into(),
        issues: vec![issue()],
    };
    let value = serde_json::to_value(&body).unwrap();
    assert_eq!(
        value,
        json!({
            "error": "deck_blocked",
            "message": "the deck cannot be played",
            "issues": [{
                "code": "copies",
                "message": "Lightning Bolt appears 5 times; at most 4 are allowed",
                "zone": "main",
                "oracle_id": "4457ed35-7c10-48c8-9776-456485fdf070",
                "name": "Lightning Bolt",
            }],
        })
    );
    assert_eq!(
        serde_json::from_value::<DeckBlockedBody>(value).unwrap(),
        body
    );
}

/// Index of each code; adding a variant breaks this match until `ALL` and the tables grow.
fn index(code: DeckChoiceErrorCode) -> usize {
    match code {
        DeckChoiceErrorCode::UnknownDeck => 0,
        DeckChoiceErrorCode::DeckBlocked => 1,
        DeckChoiceErrorCode::SeatReady => 2,
    }
}

#[test]
fn every_deck_error_code_has_one_spelling_shared_by_serde_and_as_str() {
    assert_eq!(DeckChoiceErrorCode::ALL.len(), 3);
    for (position, code) in DeckChoiceErrorCode::ALL.into_iter().enumerate() {
        assert_eq!(index(code), position, "ALL is in declaration order");
        assert_eq!(
            serde_json::to_value(code).unwrap(),
            json!(code.as_str()),
            "{code:?}"
        );
        let back: DeckChoiceErrorCode = serde_json::from_value(json!(code.as_str())).unwrap();
        assert_eq!(back, code);
    }
    assert_eq!(DeckChoiceErrorCode::UnknownDeck.as_str(), "unknown_deck");
    assert_eq!(DeckChoiceErrorCode::DeckBlocked.as_str(), "deck_blocked");
    assert_eq!(DeckChoiceErrorCode::SeatReady.as_str(), "seat_ready");
    assert!(serde_json::from_value::<DeckChoiceErrorCode>(json!("UnknownDeck")).is_err());
}

fn generated() -> BTreeMap<String, String> {
    let scratch = tempfile::tempdir().unwrap();
    export_to(scratch.path()).unwrap();
    std::fs::read_dir(scratch.path())
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(&path).unwrap(),
            )
        })
        .collect()
}

#[test]
fn the_bindings_carry_the_deck_types_and_keep_the_roster_field_optional() {
    let files = generated();
    for needed in [
        "SeatDeck.ts",
        "ChooseDeck.ts",
        "DeckBlockedBody.ts",
        "DeckChoiceErrorCode.ts",
    ] {
        assert!(files.contains_key(needed), "missing {needed}");
        assert!(files["index.ts"].contains(needed.trim_end_matches(".ts")));
    }
    for field in ["name: string", "format: string", "manual_cards: number"] {
        assert!(
            files["SeatDeck.ts"].contains(field),
            "SeatDeck lacks {field}"
        );
    }
    // Optional, so a client built before decks still compiles and a seat without one needs no field.
    assert!(
        files["LobbySeat.ts"].contains("deck?: SeatDeck"),
        "{}",
        files["LobbySeat.ts"]
    );
    assert!(files["JoinRequest.ts"].contains("deck?: string"));
    assert!(files["JoinCodeRequest.ts"].contains("deck?: string"));
    assert!(files["ChooseDeck.ts"].contains("deck: string"));
    assert!(files["DeckBlockedBody.ts"].contains("issues: Array<Issue>"));
    for code in DeckChoiceErrorCode::ALL {
        assert!(
            files["DeckChoiceErrorCode.ts"].contains(&format!("\"{}\"", code.as_str())),
            "{} missing from DeckChoiceErrorCode.ts",
            code.as_str()
        );
    }
}
