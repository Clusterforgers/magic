//! Goal: the precon bodies keep their documented wire shape: the list and one precon survive a JSON round trip, a card the database lacks is a null `card`, the boards keep their zone spelling, every precon error code has one spelling, and the TypeScript bindings carry all of it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::generated;
use mtg_cards::deck::{Counts, DeckState, DeckZone, Issue, IssueCode, Summary, ZoneCounts};
use mtg_cards::support::{Reason, Support};
use mtg_protocol::rest::{
    PRECON_DECK_PREFIX, PreconBoard, PreconCardView, PreconErrorCode, PreconList, PreconListItem,
    PreconView,
};
use rstest::rstest;
use serde_json::json;

fn summary(state: DeckState) -> Summary {
    let counts = Counts {
        total: 100,
        verified: 90,
        partial: 4,
        manual: 6,
        stale: 0,
        unplayable: 0,
    };
    Summary {
        format: "commander".to_owned(),
        state,
        counts,
        zones: vec![ZoneCounts {
            zone: DeckZone::Main,
            counts,
        }],
        issues: 0,
    }
}

fn item(code: &str, state: DeckState) -> PreconListItem {
    PreconListItem {
        code: code.to_owned(),
        name: "Adaptive Enchantment".to_owned(),
        set: "C18".to_owned(),
        deck_type: Some("Commander Deck".to_owned()),
        release_date: Some("2018-08-10".to_owned()),
        commanders: vec!["Estrid, the Masked".to_owned()],
        card_count: 100,
        format: "commander".to_owned(),
        state,
        verified: state == DeckState::Verified,
        playable: false,
        summary: summary(state),
    }
}

fn card(name: &str, count: u32, support: Support) -> PreconCardView {
    PreconCardView {
        name: name.to_owned(),
        count,
        printing_id: None,
        card: None,
        support,
        reasons: vec![Reason {
            face: 0,
            span: mtg_cards::card::Span { start: 0, end: 4 },
            text: "left to the table".to_owned(),
        }],
    }
}

fn view() -> PreconView {
    PreconView {
        precon: item("c18-adaptive-enchantment", DeckState::ManualPlay),
        issues: vec![Issue {
            code: IssueCode::NoCommanderZone,
            message: "a Normal game has no command zone".to_owned(),
            zone: Some(DeckZone::Commander),
            oracle_id: None,
            name: None,
        }],
        boards: vec![
            PreconBoard {
                board: DeckZone::Commander,
                copies: 1,
                cards: vec![card("Estrid, the Masked", 1, Support::Partial)],
            },
            PreconBoard {
                board: DeckZone::Main,
                copies: 99,
                cards: vec![card("Missing Card", 2, Support::Unplayable)],
            },
        ],
    }
}

#[test]
fn a_list_and_a_precon_survive_a_json_round_trip() {
    let list = PreconList {
        precons: vec![
            item("c18-adaptive-enchantment", DeckState::Verified),
            item("c19-merciless-rage", DeckState::Blocked),
        ],
    };
    let json = serde_json::to_value(&list).unwrap();
    assert_eq!(serde_json::from_value::<PreconList>(json).unwrap(), list);
    let one = view();
    let json = serde_json::to_value(&one).unwrap();
    assert_eq!(serde_json::from_value::<PreconView>(json).unwrap(), one);
}

#[test]
fn the_json_keys_are_the_documented_ones() {
    let json = serde_json::to_value(item("c18-x", DeckState::ManualPlay)).unwrap();
    let mut keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "card_count",
            "code",
            "commanders",
            "deck_type",
            "format",
            "name",
            "playable",
            "release_date",
            "set",
            "state",
            "summary",
            "verified"
        ]
    );
    assert_eq!(json["state"], "manual_play");
    let board = serde_json::to_value(&view().boards[0]).unwrap();
    assert_eq!(board["board"], "commander");
    assert_eq!(board["cards"][0]["card"], json!(null));
    assert_eq!(board["cards"][0]["support"], "partial");
}

/// Index of each code; adding a variant breaks this match until `ALL` and the tables grow.
fn index(code: PreconErrorCode) -> usize {
    match code {
        PreconErrorCode::NotFound => 0,
        PreconErrorCode::RateLimited => 1,
        PreconErrorCode::CardsUnavailable => 2,
        PreconErrorCode::Unavailable => 3,
        PreconErrorCode::Internal => 4,
    }
}

#[rstest]
#[case(PreconErrorCode::NotFound, "not_found")]
#[case(PreconErrorCode::RateLimited, "rate_limited")]
#[case(PreconErrorCode::CardsUnavailable, "cards_unavailable")]
#[case(PreconErrorCode::Unavailable, "unavailable")]
#[case(PreconErrorCode::Internal, "internal")]
fn every_precon_error_code_has_one_spelling_shared_by_serde_and_as_str(
    #[case] code: PreconErrorCode,
    #[case] wire: &str,
) {
    assert_eq!(code.as_str(), wire);
    assert_eq!(serde_json::to_value(code).unwrap(), json!(wire));
    assert_eq!(
        serde_json::from_value::<PreconErrorCode>(json!(wire)).unwrap(),
        code
    );
    assert_eq!(PreconErrorCode::ALL[index(code)], code);
    assert_eq!(PreconErrorCode::ALL.len(), 5);
}

#[test]
fn a_precon_is_named_as_a_deck_behind_a_prefix() {
    assert_eq!(PRECON_DECK_PREFIX, "precon:");
}

#[test]
fn the_bindings_carry_the_precon_types() {
    let files = generated();
    for needed in [
        "PreconList.ts",
        "PreconListItem.ts",
        "PreconView.ts",
        "PreconBoard.ts",
        "PreconCardView.ts",
        "PreconErrorCode.ts",
    ] {
        assert!(files.contains_key(needed), "missing {needed}");
        assert!(files["index.ts"].contains(needed.trim_end_matches(".ts")));
    }
    for field in [
        "code: string",
        "set: string",
        "commanders: Array<string>",
        "card_count: number",
        "state: DeckState",
        "playable: boolean",
        "summary: Summary",
    ] {
        assert!(
            files["PreconListItem.ts"].contains(field),
            "PreconListItem lacks {field}"
        );
    }
    assert!(files["PreconCardView.ts"].contains("card: CardHit | null"));
    assert!(files["PreconView.ts"].contains("boards: Array<PreconBoard>"));
    for code in PreconErrorCode::ALL {
        assert!(
            files["PreconErrorCode.ts"].contains(&format!("\"{}\"", code.as_str())),
            "{} missing from PreconErrorCode.ts",
            code.as_str()
        );
    }
}
