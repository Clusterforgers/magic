//! Normalized precon deck and conversion from MTGJSON's raw shape.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::raw::{RawDeck, RawDeckCard};
use crate::decklist::{DeckEntry, ParsedDeck, Section};
use crate::ids::{OracleId, PrintingId};

/// A board entry that lacks data needed to use it.
#[derive(Debug, Error)]
pub enum MtgjsonError {
    /// The input is not valid JSON or has the wrong shape.
    #[error("invalid MTGJSON: {0}")]
    Json(#[from] serde_json::Error),
    /// The deck object lacks a required field.
    #[error("deck is missing `{field}`")]
    MissingDeckField {
        /// JSON name of the missing field.
        field: &'static str,
    },
    /// A card entry lacks a required field.
    #[error("{board}[{index}] is missing `{field}`")]
    MissingCardField {
        /// JSON name of the board list.
        board: &'static str,
        /// Zero-based position within the list.
        index: usize,
        /// JSON name of the missing field.
        field: &'static str,
    },
    /// A card entry has a count of zero.
    #[error("{board}[{index}] ({name}) has a count of zero")]
    ZeroCount {
        /// JSON name of the board list.
        board: &'static str,
        /// Zero-based position within the list.
        index: usize,
        /// Card name.
        name: String,
    },
}

/// A card with its copy count in a precon deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreconCard {
    /// Number of copies.
    pub count: u32,
    /// MTGJSON card uuid.
    pub uuid: String,
    /// Card name.
    pub name: String,
    /// Set code of the printing, if given.
    pub set_code: Option<String>,
    /// Collector number of the printing, if given.
    pub number: Option<String>,
    /// Whether the copies are foil.
    pub is_foil: bool,
    /// Scryfall printing id, if MTGJSON knows it.
    pub scryfall_id: Option<PrintingId>,
    /// Scryfall oracle id, if MTGJSON knows it.
    pub oracle_id: Option<OracleId>,
}

/// An official preconstructed deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreconDeck {
    /// Deck name.
    pub name: String,
    /// Code of the set or product the deck was released with.
    pub code: String,
    /// Deck type, such as `Commander Deck`.
    pub deck_type: Option<String>,
    /// Release date as `YYYY-MM-DD`.
    pub release_date: Option<String>,
    /// Main deck cards.
    pub main_board: Vec<PreconCard>,
    /// Sideboard cards.
    pub side_board: Vec<PreconCard>,
    /// Cards that act as commanders.
    pub commander: Vec<PreconCard>,
    /// Cards featured as the deck's face; may differ from the commanders.
    pub display_commander: Vec<PreconCard>,
}

impl PreconDeck {
    /// Converts to a deck list: commanders, main deck and sideboard (display commanders are cosmetic).
    pub fn to_parsed_deck(&self) -> ParsedDeck {
        let boards = [
            (&self.commander, Section::Commander),
            (&self.main_board, Section::Main),
            (&self.side_board, Section::Sideboard),
        ];
        let entries = boards
            .into_iter()
            .flat_map(|(cards, section)| cards.iter().map(move |card| (card, section)))
            .map(|(card, section)| {
                let entry = DeckEntry::new(card.count, card.name.clone(), section);
                match &card.set_code {
                    Some(set) => entry.with_printing(set, card.number.as_deref()),
                    None => entry,
                }
            })
            .collect();
        ParsedDeck {
            entries,
            errors: Vec::new(),
            ..ParsedDeck::default()
        }
    }
}

fn card_from_raw(
    raw: RawDeckCard,
    board: &'static str,
    index: usize,
) -> Result<PreconCard, MtgjsonError> {
    let missing = |field| MtgjsonError::MissingCardField {
        board,
        index,
        field,
    };
    let name = raw.name.filter(|n| !n.is_empty()).ok_or(missing("name"))?;
    let count = raw.count.ok_or(missing("count"))?;
    if count == 0 {
        return Err(MtgjsonError::ZeroCount { board, index, name });
    }
    let uuid = raw.uuid.filter(|u| !u.is_empty()).ok_or(missing("uuid"))?;
    let identifiers = raw.identifiers.unwrap_or_default();
    Ok(PreconCard {
        count,
        uuid,
        name,
        set_code: raw.set_code.filter(|s| !s.is_empty()),
        number: raw.number.filter(|n| !n.is_empty()),
        is_foil: raw.is_foil.unwrap_or(false),
        scryfall_id: identifiers.scryfall_id.map(PrintingId),
        oracle_id: identifiers.scryfall_oracle_id.map(OracleId),
    })
}

fn board_from_raw(
    raw: Option<Vec<RawDeckCard>>,
    board: &'static str,
) -> Result<Vec<PreconCard>, MtgjsonError> {
    raw.unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(index, card)| card_from_raw(card, board, index))
        .collect()
}

impl TryFrom<RawDeck> for PreconDeck {
    type Error = MtgjsonError;

    fn try_from(raw: RawDeck) -> Result<Self, Self::Error> {
        let name = raw
            .name
            .filter(|n| !n.is_empty())
            .ok_or(MtgjsonError::MissingDeckField { field: "name" })?;
        let code = raw
            .code
            .filter(|c| !c.is_empty())
            .ok_or(MtgjsonError::MissingDeckField { field: "code" })?;
        Ok(PreconDeck {
            name,
            code,
            deck_type: raw.deck_type,
            release_date: raw.release_date,
            main_board: board_from_raw(raw.main_board, "mainBoard")?,
            side_board: board_from_raw(raw.side_board, "sideBoard")?,
            commander: board_from_raw(raw.commander, "commander")?,
            display_commander: board_from_raw(raw.display_commander, "displayCommander")?,
        })
    }
}
