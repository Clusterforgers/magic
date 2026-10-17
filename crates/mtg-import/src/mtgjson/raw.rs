//! Serde models for MTGJSON deck files.

use serde::Deserialize;

/// Cross-reference ids on an MTGJSON card.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct RawIdentifiers {
    /// Scryfall printing id.
    #[serde(rename = "scryfallId")]
    pub scryfall_id: Option<String>,
    /// Scryfall oracle id.
    #[serde(rename = "scryfallOracleId")]
    pub scryfall_oracle_id: Option<String>,
}

/// A card entry in a deck's board list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct RawDeckCard {
    /// Number of copies.
    pub count: Option<u32>,
    /// MTGJSON card uuid.
    pub uuid: Option<String>,
    /// Card name (`A // B` for multi-faced cards).
    pub name: Option<String>,
    /// Code of the set the printing belongs to.
    #[serde(rename = "setCode")]
    pub set_code: Option<String>,
    /// Collector number.
    pub number: Option<String>,
    /// Whether this copy is foil.
    #[serde(rename = "isFoil")]
    pub is_foil: Option<bool>,
    /// External ids.
    pub identifiers: Option<RawIdentifiers>,
}

/// An MTGJSON deck object.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct RawDeck {
    /// Deck name.
    pub name: Option<String>,
    /// Code of the set or product the deck belongs to.
    pub code: Option<String>,
    /// Deck type, such as `Commander Deck`.
    #[serde(rename = "type")]
    pub deck_type: Option<String>,
    /// Release date as `YYYY-MM-DD`.
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    /// Main deck cards.
    #[serde(rename = "mainBoard")]
    pub main_board: Option<Vec<RawDeckCard>>,
    /// Sideboard cards.
    #[serde(rename = "sideBoard")]
    pub side_board: Option<Vec<RawDeckCard>>,
    /// Commander cards.
    pub commander: Option<Vec<RawDeckCard>>,
    /// Cards to feature as the deck's face.
    #[serde(rename = "displayCommander")]
    pub display_commander: Option<Vec<RawDeckCard>>,
}
