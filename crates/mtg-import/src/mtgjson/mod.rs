//! MTGJSON precon deck files: raw serde models and a normalized `PreconDeck`.

mod deck;
mod deck_list;
pub mod raw;

use std::io::Read;

pub use deck::{MtgjsonError, PreconCard, PreconDeck};
pub use deck_list::{DeckListEntry, parse_deck_list};

/// Parses one MTGJSON deck file, either `{"meta":..,"data":{deck}}` or a bare deck object.
pub fn parse_deck<R: Read>(reader: R) -> Result<PreconDeck, MtgjsonError> {
    let mut value: serde_json::Value = serde_json::from_reader(reader)?;
    if let Some(data) = value.get_mut("data").filter(|d| d.is_object()) {
        value = data.take();
    }
    let raw: raw::RawDeck = serde_json::from_value(value)?;
    PreconDeck::try_from(raw)
}

/// Parses one MTGJSON deck file from a string.
pub fn parse_deck_str(json: &str) -> Result<PreconDeck, MtgjsonError> {
    parse_deck(json.as_bytes())
}

#[cfg(test)]
mod tests {
    //! Goal: MTGJSON deck files normalize into precon decks across wrapper shapes and missing optionals, and malformed files fail with a specific error.
    use super::*;
    use crate::decklist::Section;
    use crate::ids::{OracleId, PrintingId};
    use proptest::prelude::*;
    use rstest::rstest;
    use serde_json::{Value, json};

    fn card(name: &str, count: u32) -> Value {
        json!({"count": count, "uuid": format!("uuid-{name}"), "name": name, "setCode": "TST", "number": "7"})
    }

    fn deck_value(extra: Value) -> Value {
        let mut deck = json!({"name": "Test Deck", "code": "TST"});
        if let (Some(deck), Value::Object(extra)) = (deck.as_object_mut(), extra) {
            deck.extend(extra);
        }
        deck
    }

    #[rstest]
    #[case::bare(false)]
    #[case::wrapped(true)]
    fn wrapper_shapes_are_equivalent(#[case] wrapped: bool) {
        let deck = deck_value(json!({"mainBoard": [card("A", 2)]}));
        let input = if wrapped {
            json!({"meta": {"version": "5"}, "data": deck})
        } else {
            deck
        };
        let parsed = parse_deck_str(&input.to_string()).unwrap();
        assert_eq!(parsed.name, "Test Deck");
        assert_eq!(parsed.code, "TST");
        assert_eq!(parsed.main_board.len(), 1);
        assert_eq!(parsed.main_board[0].count, 2);
    }

    #[test]
    fn optional_parts_default_to_empty() {
        let parsed = parse_deck_str(&deck_value(json!({})).to_string()).unwrap();
        assert_eq!(parsed.deck_type, None);
        assert_eq!(parsed.release_date, None);
        assert!(parsed.main_board.is_empty() && parsed.side_board.is_empty());
        assert!(parsed.commander.is_empty() && parsed.display_commander.is_empty());
    }

    #[test]
    fn null_boards_and_dates_are_tolerated() {
        let input = deck_value(
            json!({"mainBoard": null, "sideBoard": null, "commander": null, "releaseDate": null, "type": null}),
        );
        let parsed = parse_deck_str(&input.to_string()).unwrap();
        assert_eq!(parsed.main_board, Vec::new());
        assert_eq!(parsed.release_date, None);
    }

    #[test]
    fn card_details_and_cross_references_are_kept() {
        let input = deck_value(json!({
            "type": "Commander Deck", "releaseDate": "2016-11-11",
            "mainBoard": [{
                "count": 1, "uuid": "u", "name": "Fire // Ice", "faceName": "Fire", "setCode": "C16",
                "number": "290", "isFoil": true, "unknownField": [1, 2],
                "identifiers": {"scryfallId": "s", "scryfallOracleId": "o", "mtgoId": "1"}
            }]
        }));
        let parsed = parse_deck_str(&input.to_string()).unwrap();
        assert_eq!(parsed.deck_type.as_deref(), Some("Commander Deck"));
        assert_eq!(parsed.release_date.as_deref(), Some("2016-11-11"));
        let card = &parsed.main_board[0];
        assert_eq!(card.name, "Fire // Ice");
        assert_eq!(
            (card.set_code.as_deref(), card.number.as_deref()),
            (Some("C16"), Some("290"))
        );
        assert!(card.is_foil);
        assert_eq!(card.scryfall_id.as_ref().map(PrintingId::as_str), Some("s"));
        assert_eq!(card.oracle_id.as_ref().map(OracleId::as_str), Some("o"));
    }

    #[test]
    fn card_without_printing_info_is_allowed() {
        let input = deck_value(json!({"mainBoard": [{"count": 1, "uuid": "u", "name": "A"}]}));
        let parsed = parse_deck_str(&input.to_string()).unwrap();
        let card = &parsed.main_board[0];
        assert_eq!((card.set_code.clone(), card.number.clone()), (None, None));
        assert!(!card.is_foil && card.oracle_id.is_none() && card.scryfall_id.is_none());
    }

    #[rstest]
    #[case::no_name(json!({"code": "T"}), "name")]
    #[case::empty_name(json!({"name": "", "code": "T"}), "name")]
    #[case::no_code(json!({"name": "N"}), "code")]
    fn deck_level_requirements(#[case] input: Value, #[case] field: &str) {
        match parse_deck_str(&input.to_string()).unwrap_err() {
            MtgjsonError::MissingDeckField { field: missing } => assert_eq!(missing, field),
            other => panic!("unexpected error {other}"),
        }
    }

    #[rstest]
    #[case::no_name("mainBoard", json!({"count": 1, "uuid": "u"}), "name")]
    #[case::no_count("mainBoard", json!({"uuid": "u", "name": "A"}), "count")]
    #[case::no_uuid("sideBoard", json!({"count": 1, "name": "A"}), "uuid")]
    #[case::empty_uuid("commander", json!({"count": 1, "uuid": "", "name": "A"}), "uuid")]
    fn card_level_requirements(#[case] board: &str, #[case] broken: Value, #[case] field: &str) {
        let input = deck_value(json!({ (board): [card("Fine", 1), broken] }));
        match parse_deck_str(&input.to_string()).unwrap_err() {
            MtgjsonError::MissingCardField {
                index,
                field: missing,
                ..
            } => {
                assert_eq!((index, missing), (1, field));
            }
            other => panic!("unexpected error {other}"),
        }
    }

    #[test]
    fn zero_count_is_rejected() {
        let input = deck_value(json!({"mainBoard": [card("A", 0)]}));
        assert!(matches!(
            parse_deck_str(&input.to_string()).unwrap_err(),
            MtgjsonError::ZeroCount { index: 0, .. }
        ));
    }

    #[rstest]
    #[case::empty("")]
    #[case::garbage("not json")]
    #[case::truncated("{\"name\": \"A\"")]
    #[case::array("[]")]
    #[case::string("\"deck\"")]
    #[case::null("null")]
    #[case::wrong_type_for_board("{\"name\":\"A\",\"code\":\"T\",\"mainBoard\":\"oops\"}")]
    #[case::negative_count(
        "{\"name\":\"A\",\"code\":\"T\",\"mainBoard\":[{\"count\":-1,\"uuid\":\"u\",\"name\":\"A\"}]}"
    )]
    #[case::fractional_count(
        "{\"name\":\"A\",\"code\":\"T\",\"mainBoard\":[{\"count\":1.5,\"uuid\":\"u\",\"name\":\"A\"}]}"
    )]
    #[case::huge_count(
        "{\"name\":\"A\",\"code\":\"T\",\"mainBoard\":[{\"count\":99999999999,\"uuid\":\"u\",\"name\":\"A\"}]}"
    )]
    fn malformed_files_are_errors(#[case] input: &str) {
        assert!(parse_deck_str(input).is_err());
    }

    #[test]
    fn data_that_is_not_an_object_is_not_unwrapped() {
        let parsed = parse_deck_str(r#"{"name":"A","code":"T","data":"ignored"}"#).unwrap();
        assert_eq!(parsed.name, "A");
    }

    #[test]
    fn conversion_orders_boards_and_ignores_display_commander() {
        let input = deck_value(json!({
            "commander": [card("Boss", 1)],
            "displayCommander": [card("Face", 1)],
            "mainBoard": [card("A", 3), card("B", 1)],
            "sideBoard": [card("S", 2)],
        }));
        let deck = parse_deck_str(&input.to_string()).unwrap().to_parsed_deck();
        assert_eq!(deck.errors, Vec::new());
        let summary: Vec<_> = deck
            .entries
            .iter()
            .map(|e| (e.name.as_str(), e.count, e.section))
            .collect();
        assert_eq!(
            summary,
            [
                ("Boss", 1, Section::Commander),
                ("A", 3, Section::Main),
                ("B", 1, Section::Main),
                ("S", 2, Section::Sideboard)
            ]
        );
        assert_eq!(deck.entries[1].set.as_deref(), Some("TST"));
        assert_eq!(deck.entries[1].collector_number.as_deref(), Some("7"));
    }

    #[test]
    fn precon_decks_roundtrip_through_serde() {
        let input = deck_value(json!({"mainBoard": [card("A", 2)], "commander": [card("B", 1)]}));
        let parsed = parse_deck_str(&input.to_string()).unwrap();
        let text = serde_json::to_string(&parsed).unwrap();
        assert_eq!(serde_json::from_str::<PreconDeck>(&text).unwrap(), parsed);
    }

    proptest! {
        #[test]
        fn counts_and_order_survive_normalization(
            counts in prop::collection::vec(1u32..=99, 0..30),
            wrapped in any::<bool>(),
        ) {
            let board: Vec<Value> = counts.iter().enumerate().map(|(i, c)| card(&format!("Card {i}"), *c)).collect();
            let deck = deck_value(json!({"mainBoard": board}));
            let input = if wrapped { json!({"data": deck}) } else { deck };
            let parsed = parse_deck_str(&input.to_string()).unwrap();
            let got: Vec<u32> = parsed.main_board.iter().map(|c| c.count).collect();
            prop_assert_eq!(got, counts.clone());
            prop_assert_eq!(parsed.to_parsed_deck().card_count(Section::Main), counts.iter().map(|c| u64::from(*c)).sum::<u64>());
        }

        #[test]
        fn arbitrary_text_never_panics(input in "\\PC{0,100}") {
            let _ = parse_deck_str(&input);
        }
    }
}
