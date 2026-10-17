//! Normalized card types, independent of Scryfall's raw JSON shape.

use std::collections::BTreeMap;

use mtg_core::color::ColorSet;
use serde::{Deserialize, Serialize};

use crate::ids::{OracleId, PrintingId};

/// Image URLs for a card or face.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageUris {
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

/// How a card's faces are laid out.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Layout {
    /// Ordinary single-faced card.
    Normal,
    /// Split card with two halves on one face.
    Split,
    /// Flip card.
    Flip,
    /// Transforming double-faced card.
    Transform,
    /// Modal double-faced card.
    ModalDfc,
    /// Meld card.
    Meld,
    /// Level up creature.
    Leveler,
    /// Class enchantment.
    Class,
    /// Case enchantment.
    Case,
    /// Saga.
    Saga,
    /// Card with an Adventure half.
    Adventure,
    /// Mutate creature.
    Mutate,
    /// Prototype artifact creature.
    Prototype,
    /// Battle.
    Battle,
    /// Plane or phenomenon.
    Planar,
    /// Scheme.
    Scheme,
    /// Vanguard avatar.
    Vanguard,
    /// Token.
    Token,
    /// Double-faced token.
    DoubleFacedToken,
    /// Emblem.
    Emblem,
    /// Augment.
    Augment,
    /// Host.
    Host,
    /// Art series card.
    ArtSeries,
    /// Reversible card with two independent fronts.
    ReversibleCard,
    /// A layout keyword this crate does not know yet.
    Other(String),
}

impl Layout {
    /// Maps a Scryfall layout keyword to a layout, keeping unknown keywords.
    pub fn parse(keyword: &str) -> Layout {
        match keyword {
            "normal" => Layout::Normal,
            "split" => Layout::Split,
            "flip" => Layout::Flip,
            "transform" => Layout::Transform,
            "modal_dfc" => Layout::ModalDfc,
            "meld" => Layout::Meld,
            "leveler" => Layout::Leveler,
            "class" => Layout::Class,
            "case" => Layout::Case,
            "saga" => Layout::Saga,
            "adventure" => Layout::Adventure,
            "mutate" => Layout::Mutate,
            "prototype" => Layout::Prototype,
            "battle" => Layout::Battle,
            "planar" => Layout::Planar,
            "scheme" => Layout::Scheme,
            "vanguard" => Layout::Vanguard,
            "token" => Layout::Token,
            "double_faced_token" => Layout::DoubleFacedToken,
            "emblem" => Layout::Emblem,
            "augment" => Layout::Augment,
            "host" => Layout::Host,
            "art_series" => Layout::ArtSeries,
            "reversible_card" => Layout::ReversibleCard,
            other => Layout::Other(other.to_owned()),
        }
    }

    /// Whether the object can be a main-deck, sideboard or commander card (unknown layouts count).
    pub fn is_deck_card(&self) -> bool {
        !matches!(
            self,
            Layout::Planar
                | Layout::Scheme
                | Layout::Vanguard
                | Layout::Token
                | Layout::DoubleFacedToken
                | Layout::Emblem
                | Layout::ArtSeries
        )
    }
}

/// Rarity of a printing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rarity {
    /// Common.
    Common,
    /// Uncommon.
    Uncommon,
    /// Rare.
    Rare,
    /// Mythic rare.
    Mythic,
    /// Special rarity.
    Special,
    /// Bonus rarity.
    Bonus,
}

impl Rarity {
    /// Parses a Scryfall rarity keyword.
    pub fn parse(keyword: &str) -> Option<Rarity> {
        match keyword {
            "common" => Some(Rarity::Common),
            "uncommon" => Some(Rarity::Uncommon),
            "rare" => Some(Rarity::Rare),
            "mythic" => Some(Rarity::Mythic),
            "special" => Some(Rarity::Special),
            "bonus" => Some(Rarity::Bonus),
            _ => None,
        }
    }
}

/// Legality of a card in one format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Legality {
    /// Legal.
    Legal,
    /// Not legal.
    NotLegal,
    /// Restricted to one copy.
    Restricted,
    /// Banned.
    Banned,
    /// A legality keyword this crate does not know.
    Unknown,
}

impl Legality {
    /// Parses a Scryfall legality keyword.
    pub fn parse(keyword: &str) -> Legality {
        match keyword {
            "legal" => Legality::Legal,
            "not_legal" => Legality::NotLegal,
            "restricted" => Legality::Restricted,
            "banned" => Legality::Banned,
            _ => Legality::Unknown,
        }
    }
}

/// Mana a card can produce.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducedMana {
    /// Colored mana types.
    pub colors: ColorSet,
    /// Whether it can produce colorless mana.
    pub colorless: bool,
}

/// One face of a card; single-faced cards have exactly one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardFace {
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
    pub colors: ColorSet,
    /// Images specific to this face, when Scryfall provides them.
    pub images: Option<ImageUris>,
}

/// A printing of a card, normalized from a Scryfall card object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardRecord {
    /// Identity of the card across printings.
    pub oracle_id: OracleId,
    /// Identity of this printing.
    pub printing_id: PrintingId,
    /// Full name, `A // B` for multi-faced cards.
    pub name: String,
    /// Face layout.
    pub layout: Layout,
    /// Mana cost of the card as a whole, falling back to the front face.
    pub mana_cost: String,
    /// Mana value (fractional for some un-cards).
    pub mana_value: f64,
    /// Type line of the whole card; face type lines joined with ` // ` when absent.
    pub type_line: String,
    /// Card colors, the union of face colors when Scryfall omits them.
    pub colors: ColorSet,
    /// Color identity.
    pub color_identity: ColorSet,
    /// Keyword abilities.
    pub keywords: Vec<String>,
    /// Mana the card can produce.
    pub produced_mana: ProducedMana,
    /// Legality per format name.
    pub legalities: BTreeMap<String, Legality>,
    /// Front face.
    pub front: CardFace,
    /// Remaining faces in Scryfall order.
    pub back_faces: Vec<CardFace>,
    /// Card-level images (absent for cards whose images live on the faces).
    pub images: Option<ImageUris>,
    /// Set code; empty if absent.
    pub set: String,
    /// Collector number; empty if absent.
    pub collector_number: String,
    /// Rarity, if recognized.
    pub rarity: Option<Rarity>,
    /// Release date as `YYYY-MM-DD`.
    pub released_at: Option<String>,
    /// Language code, `en` if absent.
    pub lang: String,
    /// Whether this printing is digital-only.
    pub digital: bool,
}

impl CardRecord {
    /// All faces, front first.
    pub fn faces(&self) -> impl Iterator<Item = &CardFace> {
        std::iter::once(&self.front).chain(&self.back_faces)
    }

    /// Whether the card has more than one face.
    pub fn is_multi_faced(&self) -> bool {
        !self.back_faces.is_empty()
    }

    /// Images for the face at `index`, falling back to card-level images.
    pub fn face_images(&self, index: usize) -> Option<&ImageUris> {
        self.faces()
            .nth(index)
            .and_then(|face| face.images.as_ref())
            .or(self.images.as_ref())
    }

    /// Legality in `format` (lowercase Scryfall name such as `commander`); unknown formats are `Unknown`.
    pub fn legality(&self, format: &str) -> Legality {
        self.legalities
            .get(format)
            .copied()
            .unwrap_or(Legality::Unknown)
    }
}

/// Who wrote a ruling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RulingSource {
    /// Wizards of the Coast.
    Wotc,
    /// Scryfall staff.
    Scryfall,
    /// A source this crate does not know, or none given.
    Unknown,
}

/// An official or editorial ruling attached to an oracle card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulingRecord {
    /// Card the ruling applies to.
    pub oracle_id: OracleId,
    /// Author of the ruling.
    pub source: RulingSource,
    /// Publication date as `YYYY-MM-DD`.
    pub published_at: Option<String>,
    /// Ruling text.
    pub comment: String,
}
