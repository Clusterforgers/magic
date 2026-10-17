//! The MTGJSON `DeckList.json` index of downloadable deck files.

use std::io::Read;

use serde::{Deserialize, Serialize};

use super::MtgjsonError;

/// One downloadable deck in the MTGJSON index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckListEntry {
    /// Code of the set or product the deck belongs to.
    pub code: String,
    /// File stem of the deck file, e.g. `VirtueAndValor_WOC`.
    #[serde(rename = "fileName")]
    pub file_name: String,
    /// Deck name.
    pub name: String,
    /// Release date as `YYYY-MM-DD`.
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    /// Deck type, such as `Commander Deck`.
    #[serde(rename = "type")]
    pub deck_type: String,
}

impl DeckListEntry {
    /// Whether the deck is a ready-to-play product that Commander players expect to find.
    pub fn is_commander_precon(&self) -> bool {
        self.deck_type == "Commander Deck"
    }
}

#[derive(Deserialize)]
struct Envelope {
    data: Vec<DeckListEntry>,
}

/// Parses `DeckList.json`, either wrapped as `{"meta":..,"data":[..]}` or a bare array.
pub fn parse_deck_list<R: Read>(mut reader: R) -> Result<Vec<DeckListEntry>, MtgjsonError> {
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| MtgjsonError::Json(serde::de::Error::custom(e)))?;
    match serde_json::from_slice::<Envelope>(&bytes) {
        Ok(envelope) => Ok(envelope.data),
        Err(_) => Ok(serde_json::from_slice(&bytes)?),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the deck index parses in both wrapper shapes, keeps every entry, and rejects structurally wrong input.
    use super::*;
    use rstest::rstest;
    use serde_json::json;

    fn entry(name: &str, deck_type: &str) -> serde_json::Value {
        json!({"code": "WOC", "fileName": format!("{name}_WOC"), "name": name, "releaseDate": "2023-09-08", "type": deck_type})
    }

    #[rstest]
    #[case::wrapped(true)]
    #[case::bare(false)]
    fn wrapper_shapes_are_equivalent(#[case] wrapped: bool) {
        let list = json!([entry("A", "Commander Deck"), entry("B", "Theme Deck")]);
        let input = if wrapped {
            json!({"meta": {"date": "2026-10-16"}, "data": list})
        } else {
            list
        };
        let decks = parse_deck_list(input.to_string().as_bytes()).unwrap();
        assert_eq!(decks.len(), 2);
        assert_eq!(decks[0].file_name, "A_WOC");
        assert!(decks[0].is_commander_precon());
        assert!(!decks[1].is_commander_precon());
    }

    #[rstest]
    #[case::not_json("nope")]
    #[case::object_without_data(r#"{"meta":{}}"#)]
    #[case::entry_missing_file_name(r#"[{"code":"X","name":"n","type":"t"}]"#)]
    fn rejects_malformed_input(#[case] input: &str) {
        assert!(parse_deck_list(input.as_bytes()).is_err());
    }

    #[test]
    fn missing_release_date_is_allowed() {
        let input = r#"[{"code":"X","fileName":"f","name":"n","type":"t"}]"#;
        assert_eq!(
            parse_deck_list(input.as_bytes()).unwrap()[0].release_date,
            None
        );
    }
}
