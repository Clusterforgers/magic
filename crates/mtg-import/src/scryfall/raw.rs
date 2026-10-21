//! Serde models for the subset of Scryfall card and ruling objects we read.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::record::ImageUris;

/// One face of a multi-faced card as Scryfall reports it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct RawCardFace {
    /// Face name.
    pub name: Option<String>,
    /// Oracle id carried on the face (reversible cards only).
    pub oracle_id: Option<String>,
    /// Mana cost of this face.
    pub mana_cost: Option<String>,
    /// Type line of this face.
    pub type_line: Option<String>,
    /// Rules text of this face.
    pub oracle_text: Option<String>,
    /// Printed power.
    pub power: Option<String>,
    /// Printed toughness.
    pub toughness: Option<String>,
    /// Starting loyalty.
    pub loyalty: Option<String>,
    /// Battle defense.
    pub defense: Option<String>,
    /// Color letters of this face.
    pub colors: Option<Vec<String>>,
    /// Images of this face.
    pub image_uris: Option<ImageUris>,
    /// Flavor name of this face on the printing.
    pub flavor_name: Option<String>,
    /// Name printed on this face when it differs from the oracle name.
    pub printed_name: Option<String>,
}

/// A card related to another one (`all_parts`): a token, a meld piece, a combo piece.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct RawRelatedCard {
    /// How the card relates: `token`, `meld_part`, `meld_result`, `combo_piece`.
    pub component: Option<String>,
    /// Name of the related card.
    pub name: Option<String>,
}

/// A Scryfall card object from Oracle Cards or Default Cards bulk data.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct RawCard {
    /// Printing id.
    pub id: Option<String>,
    /// Oracle id (absent at the root of reversible cards).
    pub oracle_id: Option<String>,
    /// Full card name, `A // B` for multi-faced cards.
    pub name: Option<String>,
    /// Language code.
    pub lang: Option<String>,
    /// Layout keyword.
    pub layout: Option<String>,
    /// Set code.
    pub set: Option<String>,
    /// Collector number within the set.
    pub collector_number: Option<String>,
    /// Rarity keyword.
    pub rarity: Option<String>,
    /// Release date as `YYYY-MM-DD`.
    pub released_at: Option<String>,
    /// Whether this printing is digital-only.
    pub digital: Option<bool>,
    /// Mana cost of the card as a whole.
    pub mana_cost: Option<String>,
    /// Mana value.
    pub cmc: Option<f64>,
    /// Type line of the card as a whole.
    pub type_line: Option<String>,
    /// Rules text.
    pub oracle_text: Option<String>,
    /// Printed power.
    pub power: Option<String>,
    /// Printed toughness.
    pub toughness: Option<String>,
    /// Starting loyalty.
    pub loyalty: Option<String>,
    /// Battle defense.
    pub defense: Option<String>,
    /// Color letters.
    pub colors: Option<Vec<String>>,
    /// Color identity letters.
    pub color_identity: Option<Vec<String>>,
    /// Keyword abilities.
    pub keywords: Option<Vec<String>>,
    /// Mana the card can produce, as letters (`C` for colorless).
    pub produced_mana: Option<Vec<String>>,
    /// Format name to legality keyword.
    pub legalities: Option<BTreeMap<String, String>>,
    /// Images of the card (absent when only faces carry images).
    pub image_uris: Option<ImageUris>,
    /// Faces of a multi-faced card.
    pub card_faces: Option<Vec<RawCardFace>>,
    /// Cards related to this one.
    pub all_parts: Option<Vec<RawRelatedCard>>,
    /// Flavor name of the printing (Secret Lair and Universes Beyond cards).
    pub flavor_name: Option<String>,
    /// Name printed on the card when it differs from the oracle name (Arena and MTGO names, translations).
    pub printed_name: Option<String>,
}

/// A Scryfall ruling object from the rulings bulk file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RawRuling {
    /// Oracle id the ruling applies to.
    pub oracle_id: Option<String>,
    /// Ruling author: `wotc` or `scryfall`.
    pub source: Option<String>,
    /// Publication date as `YYYY-MM-DD`.
    pub published_at: Option<String>,
    /// Ruling text.
    pub comment: Option<String>,
}
