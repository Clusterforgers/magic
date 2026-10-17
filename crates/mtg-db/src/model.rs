//! Cards, printings and rulings as the rest of the platform reads them.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use mtg_core::color::ColorSet;
use mtg_import::ids::{OracleId, PrintingId};
use mtg_import::scryfall::{ImageUris, Layout, Legality, ProducedMana, Rarity, RulingSource};
use serde::{Deserialize, Serialize};

use crate::colors;

/// How much of a card the rules engine implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(type_name = "impl_status", rename_all = "lowercase")]
pub enum ImplStatus {
    /// Played by hand with logged manual actions.
    Manual,
    /// Some abilities are implemented.
    Partial,
    /// Fully implemented by the engine.
    Full,
}

/// Oracle data of one card face, as stored in the `faces` column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredFace {
    /// Face name.
    pub name: String,
    /// Mana cost; empty when the face has none.
    pub mana_cost: String,
    /// Type line; empty when absent.
    pub type_line: String,
    /// Rules text; empty when absent.
    pub oracle_text: String,
    /// Printed power.
    pub power: Option<String>,
    /// Printed toughness.
    pub toughness: Option<String>,
    /// Starting loyalty.
    pub loyalty: Option<String>,
    /// Battle defense.
    pub defense: Option<String>,
    /// Colors of this face.
    #[serde(with = "colors::letters")]
    pub colors: ColorSet,
}

/// A card as identified by its oracle id, independent of printing.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    /// Identity of the card across printings.
    pub oracle_id: OracleId,
    /// Full name, `A // B` for multi-faced cards.
    pub name: String,
    /// Face layout.
    pub layout: Layout,
    /// Whether the card can be put in a deck (false for tokens, emblems and similar).
    pub is_deck_card: bool,
    /// Mana cost of the card as a whole.
    pub mana_cost: String,
    /// Mana value.
    pub mana_value: f64,
    /// Type line of the whole card.
    pub type_line: String,
    /// Rules text of all faces joined with a `//` line.
    pub oracle_text: String,
    /// Keyword abilities.
    pub keywords: Vec<String>,
    /// Card colors.
    pub colors: ColorSet,
    /// Color identity.
    pub color_identity: ColorSet,
    /// Mana the card can produce.
    pub produced_mana: ProducedMana,
    /// Legality per format name.
    pub legalities: BTreeMap<String, Legality>,
    /// Faces, front first.
    pub faces: Vec<StoredFace>,
    /// Engine coverage of the card.
    pub impl_status: ImplStatus,
}

/// Card art of one printing: the card-level images and one entry per face.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintingImages {
    /// Card-level images, absent for cards whose art lives on the faces.
    #[serde(default)]
    pub card: Option<ImageUris>,
    /// Per-face images in face order.
    #[serde(default)]
    pub faces: Vec<Option<ImageUris>>,
}

/// One printing of a card.
#[derive(Debug, Clone, PartialEq)]
pub struct Printing {
    /// Identity of this printing.
    pub printing_id: PrintingId,
    /// Card the printing belongs to.
    pub oracle_id: OracleId,
    /// Set code.
    pub set: String,
    /// Collector number within the set.
    pub collector_number: String,
    /// Rarity, if recognized.
    pub rarity: Option<Rarity>,
    /// Release date.
    pub released_at: Option<NaiveDate>,
    /// Language code.
    pub lang: String,
    /// Whether the printing is digital-only.
    pub digital: bool,
    /// Card art.
    pub images: PrintingImages,
}

/// A ruling attached to a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ruling {
    /// Card the ruling applies to.
    pub oracle_id: OracleId,
    /// Author of the ruling.
    pub source: RulingSource,
    /// Publication date.
    pub published_at: Option<NaiveDate>,
    /// Ruling text.
    pub comment: String,
}

/// A card together with everything attached to it.
#[derive(Debug, Clone, PartialEq)]
pub struct CardDetail {
    /// The card.
    pub card: Card,
    /// Printings, newest first.
    pub printings: Vec<Printing>,
    /// Rulings, oldest first.
    pub rulings: Vec<Ruling>,
}
