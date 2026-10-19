//! Bodies of the public card endpoints of `mtg-server`: `GET /api/cards` and `GET /api/cards/{oracle_id}`.

use std::collections::BTreeMap;

use mtg_cards::support::{Reason, Support};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Most cards one page of `GET /api/cards` holds; a larger `per_page` is lowered to it.
pub const MAX_PER_PAGE: u32 = 100;
/// Cards on a page when `per_page` is absent.
pub const DEFAULT_PER_PAGE: u32 = 40;
/// Highest `page` the search answers; deeper pages are refused rather than scanned.
pub const MAX_PAGE: u32 = 10_000;

/// One of the five colors, as the card pages write it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
pub enum CardColor {
    /// White.
    W,
    /// Blue.
    U,
    /// Black.
    B,
    /// Red.
    R,
    /// Green.
    G,
}

/// The pictures of one card or face, as Scryfall hosts them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ImageLinks {
    /// Small thumbnail.
    pub small: Option<String>,
    /// Standard-size image.
    pub normal: Option<String>,
    /// High-resolution image.
    pub large: Option<String>,
    /// Lossless full-card PNG.
    pub png: Option<String>,
    /// Art crop.
    pub art_crop: Option<String>,
    /// Image cropped to the card border.
    pub border_crop: Option<String>,
}

/// A printing's pictures: the card's own, or one per face for cards whose art lives on the faces.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CardImages {
    /// Card-level pictures; null when the faces carry them.
    pub card: Option<ImageLinks>,
    /// Face pictures in face order; null where a face has none.
    pub faces: Vec<Option<ImageLinks>>,
}

/// What a deck list needs to know of a card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CardHit {
    /// The Scryfall oracle id; the card across printings.
    pub oracle_id: String,
    /// Full name, `A // B` for cards with several faces.
    pub name: String,
    /// Mana cost text such as `{2}{U}`; empty when the card has none.
    pub mana_cost: String,
    /// Mana value.
    pub mana_value: f64,
    /// Type line of the whole card.
    pub type_line: String,
    /// The card's colors in WUBRG order.
    pub colors: Vec<CardColor>,
    /// The card's color identity in WUBRG order.
    pub color_identity: Vec<CardColor>,
    /// Pictures of the card's default printing.
    pub images: CardImages,
    /// Whether the game plays the card without manual steps (`docs/deck-check.md`).
    pub support: Support,
}

/// One page of `GET /api/cards`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CardSearchPage {
    /// The cards of this page, in the order of the search.
    pub cards: Vec<CardHit>,
    /// How many cards match across all pages.
    pub total: u64,
    /// This page, counted from 1.
    pub page: u32,
    /// Cards per page after the cap; the last page may hold fewer.
    pub per_page: u32,
}

/// One face of a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CardFaceView {
    /// Face name.
    pub name: String,
    /// Mana cost text; empty when the face has none.
    pub mana_cost: String,
    /// Type line of the face.
    pub type_line: String,
    /// Oracle text of the face; the `Reason` spans index into it.
    pub oracle_text: String,
    /// Printed power.
    pub power: Option<String>,
    /// Printed toughness.
    pub toughness: Option<String>,
    /// Starting loyalty.
    pub loyalty: Option<String>,
    /// Battle defense.
    pub defense: Option<String>,
    /// Colors of the face in WUBRG order.
    pub colors: Vec<CardColor>,
}

/// One printing of a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PrintingView {
    /// The Scryfall id of the printing; a deck line may name it.
    pub printing_id: String,
    /// Set code.
    pub set: String,
    /// Collector number within the set.
    pub collector_number: String,
    /// `common`, `uncommon`, `rare`, `mythic`, `special` or `bonus`; null when unknown.
    pub rarity: Option<String>,
    /// Release date as `YYYY-MM-DD`.
    pub released_at: Option<String>,
    /// Language code.
    pub lang: String,
    /// Whether the printing exists only online.
    pub digital: bool,
    /// The printing's pictures.
    pub images: CardImages,
}

/// A ruling on a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RulingView {
    /// `wotc`, `scryfall` or `unknown`.
    pub source: String,
    /// Publication date as `YYYY-MM-DD`.
    pub published_at: Option<String>,
    /// The ruling.
    pub comment: String,
}

/// The answer of `GET /api/cards/{oracle_id}`: the card with everything a preview shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CardDetail {
    /// The card as a search hit shows it.
    pub card: CardHit,
    /// Rules text of all faces joined with a `//` line.
    pub oracle_text: String,
    /// Keyword abilities and ability words.
    pub keywords: Vec<String>,
    /// The faces, front first.
    pub faces: Vec<CardFaceView>,
    /// Scryfall's legality per format: `legal`, `not_legal`, `restricted` or `banned`.
    pub legalities: BTreeMap<String, String>,
    /// The printings, newest first.
    pub printings: Vec<PrintingView>,
    /// The rulings, oldest first.
    pub rulings: Vec<RulingView>,
    /// The clauses the game cannot run, each over a span of one face's oracle text; empty for a
    /// card that is `verified`.
    pub reasons: Vec<Reason>,
}
