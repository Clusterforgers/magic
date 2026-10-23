//! Goal: the deck bodies keep their documented JSON shape and refuse what a client may not send, the check report and the stored check travel exactly as `docs/deck-check.md` defines them, every error code has one spelling shared by serde and `as_str`, and the TypeScript bindings carry the deck types with their unions, and a check may name the game or the rules of the table that judges the deck.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common::generated;
use mtg_cards::deck::{
    Counts, DeckReport, DeckState, DeckZone, Issue, IssueCode, ReportCard, StoredCheck, Summary,
    ZoneCounts,
};
use mtg_cards::support::Support;
use mtg_protocol::rest::{
    CardSuggestion, CheckRequest, DeckBody, DeckErrorCode, DeckLineBody, DeckList, DeckListItem,
    ImportRequest, ImportSource, MAX_DECK_CARDS, MAX_DECK_LINES, MAX_DECK_NAME_CHARS,
    MAX_DECK_NOTES_CHARS, MAX_DECKS_PER_USER, MAX_IMPORT_CHARS, MAX_IMPORT_LINES,
    PRECON_DECK_PREFIX, UnresolvedLine, UnresolvedReason,
};
use rstest::rstest;
use serde_json::{Value, json};

const ID: &str = "4457ed35-7c10-48c8-9776-456485fdf070";

fn summary() -> Summary {
    let counts = Counts {
        total: 61,
        verified: 60,
        partial: 1,
        ..Counts::default()
    };
    Summary {
        format: "modern".into(),
        state: DeckState::ManualPlay,
        counts,
        zones: vec![ZoneCounts {
            zone: DeckZone::Main,
            counts,
        }],
        issues: 1,
    }
}

fn report() -> DeckReport {
    DeckReport {
        deck_hash: "d".repeat(64),
        coverage_hash: "c".repeat(64),
        format: "modern".into(),
        legality: vec![Issue {
            code: IssueCode::Banned,
            message: "Lightning Bolt is banned in modern".into(),
            zone: Some(DeckZone::Main),
            oracle_id: Some(ID.into()),
            name: Some("Lightning Bolt".into()),
        }],
        cards: vec![ReportCard {
            oracle_id: ID.into(),
            name: "Lightning Bolt".into(),
            zone: DeckZone::Main,
            count: 4,
            support: Support::Verified,
            reasons: vec![],
        }],
        summary: summary(),
    }
}

#[test]
fn a_deck_body_has_the_documented_wire_shape() {
    let body = DeckBody {
        name: "Burn".into(),
        format: "modern".into(),
        notes: "notes".into(),
        cards: vec![
            DeckLineBody {
                zone: DeckZone::Main,
                oracle_id: ID.into(),
                printing_id: Some("e3285e6b-3e79-4d7c-bf96-d920f973b122".into()),
                count: 4,
            },
            DeckLineBody {
                zone: DeckZone::Companion,
                oracle_id: ID.into(),
                printing_id: None,
                count: 1,
            },
        ],
    };
    let value = serde_json::to_value(&body).unwrap();
    assert_eq!(
        value,
        json!({
            "name": "Burn", "format": "modern", "notes": "notes",
            "cards": [
                {"zone": "main", "oracle_id": ID, "printing_id": "e3285e6b-3e79-4d7c-bf96-d920f973b122", "count": 4},
                {"zone": "companion", "oracle_id": ID, "printing_id": null, "count": 1}
            ]
        })
    );
    assert_eq!(serde_json::from_value::<DeckBody>(value).unwrap(), body);
}

#[test]
fn notes_and_printings_may_be_left_out_of_a_request() {
    let body: DeckBody = serde_json::from_value(json!({
        "name": "Burn", "format": "modern",
        "cards": [{"zone": "main", "oracle_id": ID, "count": 4}]
    }))
    .unwrap();
    assert_eq!(body.notes, "");
    assert_eq!(body.cards[0].printing_id, None);
}

#[rstest]
#[case::another_field(json!({"name": "a", "format": "modern", "cards": [], "owner": "me"}))]
#[case::no_name(json!({"format": "modern", "cards": []}))]
#[case::no_format(json!({"name": "a", "cards": []}))]
#[case::no_cards(json!({"name": "a", "format": "modern"}))]
#[case::a_bad_zone(json!({"name": "a", "format": "modern", "cards": [{"zone": "graveyard", "oracle_id": ID, "count": 1}]}))]
#[case::a_negative_count(json!({"name": "a", "format": "modern", "cards": [{"zone": "main", "oracle_id": ID, "count": -1}]}))]
#[case::a_field_of_a_line(json!({"name": "a", "format": "modern", "cards": [{"zone": "main", "oracle_id": ID, "count": 1, "foil": true}]}))]
#[case::cards_that_are_text(json!({"name": "a", "format": "modern", "cards": "4 Bolt"}))]
fn bodies_that_are_not_exactly_a_deck_are_refused(#[case] body: Value) {
    assert!(serde_json::from_value::<DeckBody>(body).is_err());
}

#[test]
fn the_check_request_takes_a_deck_or_a_list_and_nothing_else() {
    let by_id: CheckRequest = serde_json::from_value(json!({"deck": ID})).unwrap();
    assert_eq!(
        (by_id.deck.as_deref(), by_id.format, by_id.cards),
        (Some(ID), None, None)
    );
    let inline: CheckRequest = serde_json::from_value(json!({
        "format": "commander", "cards": [{"zone": "commander", "oracle_id": ID, "count": 1}]
    }))
    .unwrap();
    assert_eq!(inline.deck, None);
    assert_eq!(inline.cards.unwrap().len(), 1);
    assert!(serde_json::from_value::<CheckRequest>(json!({"deck": ID, "name": "x"})).is_err());
}

#[test]
fn the_check_request_may_name_the_table_that_judges_the_deck() {
    let by_game: CheckRequest = serde_json::from_value(json!({"deck": ID, "game": "g-1"})).unwrap();
    assert_eq!(by_game.game.as_deref(), Some("g-1"));
    assert_eq!(by_game.table, None);
    let described: CheckRequest = serde_json::from_value(json!({
        "deck": ID,
        "table": {"format": "commander", "house_rules": {"deck": {"singleton": false}}}
    }))
    .unwrap();
    let table = described.table.unwrap();
    assert_eq!(table.format, "commander");
    assert_eq!(table.house_rules.unwrap().deck.singleton, Some(false));
    let bare: CheckRequest =
        serde_json::from_value(json!({"deck": ID, "table": {"format": "normal"}})).unwrap();
    assert_eq!(bare.table.unwrap().house_rules, None);
    // Left out of the JSON while absent, and a rule the table does not know is refused.
    let plain: CheckRequest = serde_json::from_value(json!({"deck": ID})).unwrap();
    let value = serde_json::to_value(&plain).unwrap();
    assert!(value.get("game").is_none() && value.get("table").is_none());
    for body in [
        json!({"deck": ID, "table": {}}),
        json!({"deck": ID, "table": {"format": "normal", "banana": 1}}),
        json!({"deck": ID, "table": {"format": "commander", "house_rules": {"banana": 1}}}),
        json!({"deck": ID, "game": 7}),
    ] {
        assert!(serde_json::from_value::<CheckRequest>(body).is_err());
    }
}

#[rstest]
#[case::alone(json!({"deck": "precon:c18-adaptive-enchantment"}))]
#[case::at_a_game(json!({"deck": "precon:c18-adaptive-enchantment", "game": "g-1"}))]
#[case::at_a_described_table(json!({"deck": "precon:c18-adaptive-enchantment", "table": {"format": "commander"}}))]
#[case::at_a_table_with_rules(json!({"deck": "precon:c18-adaptive-enchantment", "table": {"format": "commander", "house_rules": {"deck": {"singleton": false}}}}))]
fn the_check_request_names_a_precon_as_its_deck_by_the_prefix(#[case] body: Value) {
    let request: CheckRequest = serde_json::from_value(body).unwrap();
    let deck = request.deck.as_deref().unwrap();
    assert_eq!(
        deck.strip_prefix(PRECON_DECK_PREFIX),
        Some("c18-adaptive-enchantment")
    );
    assert_eq!((request.format, request.cards), (None, None));
}

#[test]
fn an_import_request_names_a_list_and_may_name_its_kind_and_format() {
    let plain: ImportRequest = serde_json::from_value(json!({"text": "4 Bolt"})).unwrap();
    assert_eq!((plain.source, plain.format), (None, None));
    let full: ImportRequest =
        serde_json::from_value(json!({"text": "x", "source": "mtgo_dek", "format": "commander"}))
            .unwrap();
    assert_eq!(full.source, Some(ImportSource::MtgoDek));
    assert!(serde_json::from_value::<ImportRequest>(json!({"source": "text"})).is_err());
    assert!(
        serde_json::from_value::<ImportRequest>(json!({"text": "x", "source": "excel"})).is_err()
    );
}

#[rstest]
#[case(ImportSource::Text, "text")]
#[case(ImportSource::Arena, "arena")]
#[case(ImportSource::Mtgo, "mtgo")]
#[case(ImportSource::MtgoDek, "mtgo_dek")]
#[case(ImportSource::Moxfield, "moxfield")]
#[case(ImportSource::Archidekt, "archidekt")]
fn sources_are_snake_case_words(#[case] source: ImportSource, #[case] word: &str) {
    assert_eq!(serde_json::to_value(source).unwrap(), json!(word));
    assert_eq!(
        serde_json::from_value::<ImportSource>(json!(word)).unwrap(),
        source
    );
}

#[test]
fn an_unresolved_line_and_a_list_item_have_the_documented_wire_shape() {
    let line = UnresolvedLine {
        name: "lightnin bolt".into(),
        count: 2,
        zone: DeckZone::Sideboard,
        reason: UnresolvedReason::UnknownCard,
        message: "no card is named \"lightnin bolt\"".into(),
        suggestions: vec![CardSuggestion {
            oracle_id: ID.into(),
            name: "Lightning Bolt".into(),
        }],
    };
    assert_eq!(
        serde_json::to_value(&line).unwrap(),
        json!({
            "name": "lightnin bolt", "count": 2, "zone": "sideboard", "reason": "unknown_card",
            "message": "no card is named \"lightnin bolt\"",
            "suggestions": [{"oracle_id": ID, "name": "Lightning Bolt"}]
        })
    );
    let ambiguous = serde_json::to_value(UnresolvedReason::AmbiguousName).unwrap();
    assert_eq!(ambiguous, json!("ambiguous_name"));

    let item = DeckListItem {
        id: ID.into(),
        name: "Burn".into(),
        format: "modern".into(),
        created_at: "2026-10-19T12:00:00Z".into(),
        updated_at: "2026-10-19T12:30:00Z".into(),
        state: DeckState::Unchecked,
        verified: false,
        changed: true,
        checked_at: Some("2026-10-19T12:00:00Z".into()),
        summary: summary(),
    };
    let value = serde_json::to_value(DeckList {
        decks: vec![item.clone()],
    })
    .unwrap();
    assert_eq!(value["decks"][0]["state"], "unchecked");
    assert_eq!(value["decks"][0]["verified"], false);
    assert_eq!(value["decks"][0]["changed"], true);
    assert_eq!(value["decks"][0]["summary"]["state"], "manual_play");
    assert_eq!(
        serde_json::from_value::<DeckList>(value).unwrap().decks,
        [item]
    );
}

#[test]
fn the_report_and_the_stored_check_travel_as_the_design_defines_them() {
    let report = report();
    let value = serde_json::to_value(&report).unwrap();
    let keys: BTreeSet<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from([
            "cards",
            "coverage_hash",
            "deck_hash",
            "format",
            "legality",
            "summary"
        ])
    );
    assert_eq!(
        value["legality"][0],
        json!({
            "code": "banned", "message": "Lightning Bolt is banned in modern",
            "zone": "main", "oracle_id": ID, "name": "Lightning Bolt"
        })
    );
    assert_eq!(
        value["cards"][0],
        json!({
            "oracle_id": ID, "name": "Lightning Bolt", "zone": "main", "count": 4,
            "support": "verified", "reasons": []
        })
    );
    assert_eq!(serde_json::from_value::<DeckReport>(value).unwrap(), report);

    let stored = StoredCheck::of(&report, "2026-10-19T12:00:00Z");
    let value = serde_json::to_value(&stored).unwrap();
    let keys: BTreeSet<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from(["checked_at", "coverage_hash", "deck_hash", "summary"])
    );
    assert_eq!(
        serde_json::from_value::<StoredCheck>(value).unwrap(),
        stored
    );
}

#[rstest]
#[case(DeckState::Verified, "verified")]
#[case(DeckState::ManualPlay, "manual_play")]
#[case(DeckState::Blocked, "blocked")]
#[case(DeckState::Unchecked, "unchecked")]
fn deck_states_are_snake_case_words(#[case] state: DeckState, #[case] word: &str) {
    assert_eq!(serde_json::to_value(state).unwrap(), json!(word));
    assert_eq!(
        serde_json::from_value::<DeckState>(json!(word)).unwrap(),
        state
    );
}

#[rstest]
#[case(DeckZone::Main, "main")]
#[case(DeckZone::Sideboard, "sideboard")]
#[case(DeckZone::Commander, "commander")]
#[case(DeckZone::Companion, "companion")]
fn zones_are_lower_case_words(#[case] zone: DeckZone, #[case] word: &str) {
    assert_eq!(serde_json::to_value(zone).unwrap(), json!(word));
    assert_eq!(zone.keyword(), word);
}

/// Index of each code; adding a variant breaks this match until `ALL` and the tables grow.
fn index(code: DeckErrorCode) -> usize {
    match code {
        DeckErrorCode::InvalidRequest => 0,
        DeckErrorCode::InvalidDeck => 1,
        DeckErrorCode::UnknownCards => 2,
        DeckErrorCode::NotFound => 3,
        DeckErrorCode::DeckLimit => 4,
        DeckErrorCode::ExportFailed => 5,
        DeckErrorCode::Unauthenticated => 6,
        DeckErrorCode::ForbiddenOrigin => 7,
        DeckErrorCode::PayloadTooLarge => 8,
        DeckErrorCode::UnsupportedMediaType => 9,
        DeckErrorCode::DecksUnavailable => 10,
        DeckErrorCode::CardsUnavailable => 11,
        DeckErrorCode::Unavailable => 12,
        DeckErrorCode::Internal => 13,
        DeckErrorCode::RateLimited => 14,
    }
}

#[test]
fn error_codes_have_one_spelling_for_serde_and_as_str() {
    let mut seen = BTreeSet::new();
    for (position, code) in DeckErrorCode::ALL.into_iter().enumerate() {
        assert_eq!(index(code), position, "ALL is in declaration order");
        let text = code.as_str();
        assert_eq!(serde_json::to_value(code).unwrap(), json!(text));
        assert_eq!(
            serde_json::from_value::<DeckErrorCode>(json!(text)).unwrap(),
            code
        );
        assert!(
            text.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
            "{text} is snake_case"
        );
        assert!(seen.insert(text), "{text} appears twice");
    }
    assert_eq!(seen.len(), DeckErrorCode::ALL.len());
}

#[test]
fn the_limits_match_the_documented_numbers() {
    assert_eq!(MAX_DECKS_PER_USER, 100);
    assert_eq!((MAX_DECK_LINES, MAX_DECK_CARDS), (250, 250));
    assert_eq!((MAX_DECK_NAME_CHARS, MAX_DECK_NOTES_CHARS), (100, 10_000));
    assert_eq!((MAX_IMPORT_CHARS, MAX_IMPORT_LINES), (100_000, 1_000));
    const { assert!(MAX_DECK_LINES <= MAX_DECK_CARDS as usize) };
}

#[test]
fn the_bindings_carry_the_deck_types() {
    let files = generated();
    for needed in [
        "DeckBody.ts",
        "DeckLineBody.ts",
        "DeckView.ts",
        "DeckLineView.ts",
        "DeckList.ts",
        "DeckListItem.ts",
        "CheckRequest.ts",
        "TableRules.ts",
        "ImportRequest.ts",
        "ImportResult.ts",
        "ImportSource.ts",
        "UnresolvedLine.ts",
        "UnresolvedReason.ts",
        "CardSuggestion.ts",
        "ListProblem.ts",
        "IgnoredCards.ts",
        "DeckErrorCode.ts",
        "DeckReport.ts",
        "DeckState.ts",
        "DeckZone.ts",
        "StoredCheck.ts",
        "Summary.ts",
        "Counts.ts",
        "ZoneCounts.ts",
        "ReportCard.ts",
        "Issue.ts",
        "IssueCode.ts",
    ] {
        assert!(files.contains_key(needed), "missing {needed}");
        assert!(files["index.ts"].contains(needed.trim_end_matches(".ts")));
    }
    assert!(
        files["DeckZone.ts"].contains("\"main\" | \"sideboard\" | \"commander\" | \"companion\"")
    );
    assert!(
        files["DeckState.ts"]
            .contains("\"verified\" | \"manual_play\" | \"blocked\" | \"unchecked\"")
    );
    assert!(files["DeckView.ts"].contains("state: DeckState"));
    assert!(files["DeckView.ts"].contains("check: StoredCheck | null"));
    assert!(files["DeckView.ts"].contains("report: DeckReport"));
    assert!(files["DeckView.ts"].contains("cards: Array<DeckLineView>"));
    assert!(files["DeckReport.ts"].contains("legality: Array<Issue>"));
    assert!(files["DeckReport.ts"].contains("cards: Array<ReportCard>"));
    assert!(files["StoredCheck.ts"].contains("checked_at: string"));
    assert!(files["ImportResult.ts"].contains("unresolved: Array<UnresolvedLine>"));
    assert!(files["ImportSource.ts"].contains("\"mtgo_dek\""));
    for code in DeckErrorCode::ALL {
        assert!(
            files["DeckErrorCode.ts"].contains(&format!("\"{}\"", code.as_str())),
            "{} missing from DeckErrorCode.ts",
            code.as_str()
        );
    }
}
