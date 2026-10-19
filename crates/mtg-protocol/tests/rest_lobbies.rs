//! Goal: the game browser bodies keep their documented JSON shape and refuse anything else, the lookup of a game says who may find it, a seat token never reaches `Debug` output, visibility is invite-only unless it says otherwise, every error code has one spelling shared by serde and `as_str`, and the TypeScript bindings carry all of it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use mtg_protocol::bindings::export_to;
use mtg_protocol::rest::{
    GameInfo, GameSeat, JoinCodeRequest, JoinRequest, Joined, LobbyErrorCode, LobbyGame, LobbyList,
    Visibility, VisibilityBody,
};
use rstest::rstest;
use serde_json::{Value, json};

/// Index of each code; adding a variant breaks this match until `ALL` and the tables grow.
fn index(code: LobbyErrorCode) -> usize {
    match code {
        LobbyErrorCode::InvalidRequest => 0,
        LobbyErrorCode::InvalidName => 1,
        LobbyErrorCode::Unauthenticated => 2,
        LobbyErrorCode::ForbiddenOrigin => 3,
        LobbyErrorCode::Forbidden => 4,
        LobbyErrorCode::NotFound => 5,
        LobbyErrorCode::Full => 6,
        LobbyErrorCode::Started => 7,
        LobbyErrorCode::NotClaimed => 8,
        LobbyErrorCode::PayloadTooLarge => 9,
        LobbyErrorCode::UnsupportedMediaType => 10,
        LobbyErrorCode::TooManyAttempts => 11,
        LobbyErrorCode::RateLimited => 12,
        LobbyErrorCode::AccountsUnavailable => 13,
        LobbyErrorCode::LobbiesUnavailable => 14,
        LobbyErrorCode::Unavailable => 15,
        LobbyErrorCode::Internal => 16,
    }
}

fn game(host: &str, filled: u32) -> LobbyGame {
    LobbyGame {
        game: "6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55".into(),
        host: host.into(),
        format: "normal".into(),
        filled,
        total: 4,
        created_at: "2026-10-19T12:30:00Z".into(),
        age_secs: 125,
    }
}

#[test]
fn the_listing_has_the_documented_wire_shape() {
    let list = LobbyList {
        games: vec![game("Ann", 1), game("Bo", 3)],
    };
    let value = serde_json::to_value(&list).unwrap();
    assert_eq!(
        value["games"][0],
        json!({
            "game": "6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55",
            "host": "Ann",
            "format": "normal",
            "filled": 1,
            "total": 4,
            "created_at": "2026-10-19T12:30:00Z",
            "age_secs": 125,
        })
    );
    assert_eq!(value["games"].as_array().unwrap().len(), 2);
    assert_eq!(serde_json::from_value::<LobbyList>(value).unwrap(), list);
}

#[test]
fn an_empty_listing_is_an_empty_list() {
    let empty = LobbyList { games: vec![] };
    assert_eq!(serde_json::to_value(&empty).unwrap(), json!({"games": []}));
}

/// A game of two seats of which the second is open, as the lookup shows it.
fn lookup(started: bool, result: Option<GameResult>, visibility: Visibility) -> GameInfo {
    GameInfo {
        started,
        seats: vec![
            GameSeat {
                seat: PlayerId(0),
                name: "Ann".into(),
            },
            GameSeat {
                seat: PlayerId(1),
                name: String::new(),
            },
        ],
        connected: vec![PlayerId(0)],
        ready: vec![PlayerId(0)],
        result,
        visibility,
    }
}

#[test]
fn the_game_lookup_has_the_documented_wire_shape() {
    let waiting = lookup(false, None, Visibility::Public);
    let value = serde_json::to_value(&waiting).unwrap();
    assert_eq!(
        value,
        json!({
            "started": false,
            "seats": [{"seat": 0, "name": "Ann"}, {"seat": 1, "name": ""}],
            "connected": [0],
            "ready": [0],
            "result": null,
            "visibility": "public",
        })
    );
    assert_eq!(serde_json::from_value::<GameInfo>(value).unwrap(), waiting);

    let over = lookup(
        true,
        Some(GameResult::Winner(PlayerId(1))),
        Visibility::Invite,
    );
    let value = serde_json::to_value(&over).unwrap();
    assert_eq!(value["result"], json!({"Winner": 1}));
    assert_eq!(value["visibility"], "invite");
    assert_eq!(serde_json::from_value::<GameInfo>(value).unwrap(), over);
}

#[rstest]
#[case(Visibility::Public, "public")]
#[case(Visibility::Invite, "invite")]
fn the_lookup_carries_the_visibility_in_force(#[case] visibility: Visibility, #[case] text: &str) {
    for started in [false, true] {
        let value = serde_json::to_value(lookup(started, None, visibility)).unwrap();
        assert_eq!(value["visibility"], json!(text));
    }
}

#[rstest]
#[case::missing(None)]
#[case::null(Some(json!(null)))]
#[case::wrong_case(Some(json!("Public")))]
#[case::unknown_word(Some(json!("private")))]
#[case::a_flag(Some(json!(true)))]
fn a_lookup_must_name_a_visibility_it_knows(#[case] visibility: Option<Value>) {
    let mut value = serde_json::to_value(lookup(false, None, Visibility::Invite)).unwrap();
    let body = value.as_object_mut().unwrap();
    match visibility {
        Some(other) => body.insert("visibility".into(), other),
        None => body.remove("visibility"),
    };
    assert!(serde_json::from_value::<GameInfo>(value).is_err());
}

#[rstest]
#[case::no_name(json!({}), None)]
#[case::a_name(json!({"name": "Bo"}), Some("Bo"))]
#[case::null_is_absent(json!({"name": null}), None)]
#[case::unicode(json!({"name": "Zoë 日本語"}), Some("Zoë 日本語"))]
fn a_join_request_takes_an_optional_name(#[case] body: Value, #[case] name: Option<&str>) {
    let request = serde_json::from_value::<JoinRequest>(body).unwrap();
    assert_eq!(request.name.as_deref(), name);
    let by_code =
        serde_json::from_value::<JoinCodeRequest>(json!({"code": "ABCD2345", "name": name}));
    assert_eq!(by_code.unwrap().name.as_deref(), name);
}

#[rstest]
#[case::name_is_a_number(json!({"name": 5}))]
#[case::name_is_a_list(json!({"name": ["Bo"]}))]
#[case::unknown_field(json!({"name": "Bo", "seat": 1}))]
#[case::a_token(json!({"token": "x"}))]
#[case::not_an_object(json!("Bo"))]
fn a_join_request_refuses_everything_else(#[case] body: Value) {
    assert!(serde_json::from_value::<JoinRequest>(body).is_err());
}

#[rstest]
#[case::no_code(json!({"name": "Bo"}))]
#[case::code_is_a_number(json!({"code": 12_345_678}))]
#[case::code_is_null(json!({"code": null}))]
#[case::unknown_field(json!({"code": "ABCD2345", "game": "x"}))]
#[case::empty(json!({}))]
fn a_join_code_request_needs_a_text_code_and_refuses_unknown_fields(#[case] body: Value) {
    assert!(serde_json::from_value::<JoinCodeRequest>(body).is_err());
}

#[test]
fn an_absent_name_is_left_out_of_a_request() {
    let request = JoinRequest { name: None };
    assert_eq!(serde_json::to_value(&request).unwrap(), json!({}));
    let named = JoinCodeRequest {
        code: "ABCD2345".into(),
        name: Some("Bo".into()),
    };
    assert_eq!(
        serde_json::to_value(&named).unwrap(),
        json!({"code": "ABCD2345", "name": "Bo"})
    );
    let bare = JoinCodeRequest {
        code: "ABCD2345".into(),
        name: None,
    };
    assert_eq!(
        serde_json::to_value(&bare).unwrap(),
        json!({"code": "ABCD2345"})
    );
}

#[test]
fn a_join_answers_with_the_seat_and_a_token_that_may_be_null() {
    let fresh = Joined {
        seat: PlayerId(2),
        token: Some("t0k3n".into()),
    };
    assert_eq!(
        serde_json::to_value(&fresh).unwrap(),
        json!({"seat": 2, "token": "t0k3n"})
    );
    let again = Joined {
        seat: PlayerId(2),
        token: None,
    };
    let value = serde_json::to_value(&again).unwrap();
    assert_eq!(value, json!({"seat": 2, "token": null}));
    assert_eq!(serde_json::from_value::<Joined>(value).unwrap(), again);
}

#[test]
fn debug_output_hides_the_token() {
    let joined = Joined {
        seat: PlayerId(1),
        token: Some("0123456789abcdef".into()),
    };
    let shown = format!("{joined:?} {joined:#?}");
    assert!(!shown.contains("0123456789abcdef"), "{shown}");
    assert!(shown.contains("<redacted>"), "{shown}");
    let without = format!(
        "{:?}",
        Joined {
            seat: PlayerId(1),
            token: None
        }
    );
    assert!(without.contains("None"), "{without}");
}

#[rstest]
#[case(Visibility::Public, "public")]
#[case(Visibility::Invite, "invite")]
fn visibility_is_spelled_in_snake_case(#[case] visibility: Visibility, #[case] text: &str) {
    assert_eq!(visibility.as_str(), text);
    assert_eq!(serde_json::to_value(visibility).unwrap(), json!(text));
    assert_eq!(
        serde_json::from_value::<Visibility>(json!(text)).unwrap(),
        visibility
    );
    assert!(Visibility::ALL.contains(&visibility));
}

#[rstest]
#[case("Public")]
#[case("PUBLIC")]
#[case("private")]
#[case("")]
fn visibility_refuses_other_spellings(#[case] text: &str) {
    assert!(serde_json::from_value::<Visibility>(json!(text)).is_err());
}

#[test]
fn a_game_is_invite_only_unless_it_says_otherwise() {
    assert_eq!(Visibility::default(), Visibility::Invite);
}

#[rstest]
#[case::public(json!({"visibility": "public"}), true)]
#[case::invite(json!({"visibility": "invite"}), true)]
#[case::missing(json!({}), false)]
#[case::null(json!({"visibility": null}), false)]
#[case::wrong_case(json!({"visibility": "Public"}), false)]
#[case::unknown_field(json!({"visibility": "public", "held": 1}), false)]
fn the_visibility_body_needs_exactly_a_visibility(#[case] body: Value, #[case] accepted: bool) {
    assert_eq!(
        serde_json::from_value::<VisibilityBody>(body).is_ok(),
        accepted
    );
}

#[test]
fn every_error_code_has_one_spelling_for_serde_and_as_str() {
    let mut seen = BTreeSet::new();
    for (position, code) in LobbyErrorCode::ALL.into_iter().enumerate() {
        assert_eq!(
            index(code),
            position,
            "{} is out of order in ALL",
            code.as_str()
        );
        assert_eq!(serde_json::to_value(code).unwrap(), json!(code.as_str()));
        assert_eq!(
            serde_json::from_value::<LobbyErrorCode>(json!(code.as_str())).unwrap(),
            code
        );
        let text = code.as_str();
        assert!(
            text.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
            "{text} is not snake_case"
        );
        assert!(seen.insert(text), "{text} is spelled twice");
    }
    assert_eq!(seen.len(), 17);
}

#[test]
fn the_error_codes_share_their_spelling_with_the_other_endpoints() {
    // The same word means the same thing wherever the server sends it.
    for shared in [
        "invalid_request",
        "unauthenticated",
        "forbidden_origin",
        "not_found",
        "payload_too_large",
        "unsupported_media_type",
        "too_many_attempts",
        "accounts_unavailable",
        "unavailable",
        "internal",
    ] {
        assert!(
            LobbyErrorCode::ALL
                .iter()
                .any(|code| code.as_str() == shared),
            "{shared} is missing"
        );
    }
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
fn the_bindings_carry_the_game_browser_types() {
    let files = generated();
    for needed in [
        "LobbyList.ts",
        "LobbyGame.ts",
        "GameInfo.ts",
        "GameSeat.ts",
        "JoinRequest.ts",
        "JoinCodeRequest.ts",
        "Joined.ts",
        "Visibility.ts",
        "VisibilityBody.ts",
        "LobbyErrorCode.ts",
    ] {
        assert!(files.contains_key(needed), "missing {needed}");
        assert!(files["index.ts"].contains(needed.trim_end_matches(".ts")));
    }
    assert!(files["Visibility.ts"].contains("\"public\" | \"invite\""));
    assert!(files["LobbyList.ts"].contains("games: Array<LobbyGame>"));
    for field in [
        "game: string",
        "host: string",
        "format: string",
        "filled: number",
        "total: number",
        "created_at: string",
        "age_secs: number",
    ] {
        assert!(files["LobbyGame.ts"].contains(field), "{field}");
    }
    // A request leaves its name out when there is none; an answer says null for a token it does not give.
    assert!(files["JoinRequest.ts"].contains("name?: string"));
    assert!(files["JoinCodeRequest.ts"].contains("code: string"));
    assert!(files["JoinCodeRequest.ts"].contains("name?: string"));
    assert!(files["Joined.ts"].contains("seat: PlayerId"));
    assert!(files["Joined.ts"].contains("token: string | null"));
    assert!(files["VisibilityBody.ts"].contains("visibility: Visibility"));
    for field in [
        "started: boolean",
        "seats: Array<GameSeat>",
        "connected: Array<PlayerId>",
        "ready: Array<PlayerId>",
        "result: GameResult | null",
        "visibility: Visibility",
    ] {
        assert!(files["GameInfo.ts"].contains(field), "{field}");
    }
    assert!(files["GameSeat.ts"].contains("seat: PlayerId"));
    assert!(files["GameSeat.ts"].contains("name: string"));
    for code in LobbyErrorCode::ALL {
        assert!(
            files["LobbyErrorCode.ts"].contains(&format!("\"{}\"", code.as_str())),
            "{} missing from LobbyErrorCode.ts",
            code.as_str()
        );
    }
}

#[test]
fn the_lobby_roster_says_an_open_seat_has_an_empty_name() {
    let files = generated();
    assert!(
        files["LobbySeat.ts"].contains("empty for an open seat"),
        "{}",
        files["LobbySeat.ts"]
    );
}
