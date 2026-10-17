//! Raw result rows and their conversion into public model types.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use mtg_import::ids::{OracleId, PrintingId};
use mtg_import::scryfall::{Layout, Legality, ProducedMana, Rarity};
use sqlx::types::Json;
use uuid::Uuid;

use crate::codec::ruling_source_from_keyword;
use crate::colors::from_mask;
use crate::error::DbError;
use crate::model::{Card, ImplStatus, Printing, PrintingImages, Ruling, StoredFace};

/// Columns of [`CardRow`], in the order the struct declares them.
pub(crate) const CARD_COLUMNS: &str = "oracle_id, name, layout, is_deck_card, mana_cost, mana_value, type_line, oracle_text, keywords, colors, color_identity, produced_mana, produces_colorless, legalities, faces, impl_status";

/// A `cards` row as selected by every card query.
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct CardRow {
    pub oracle_id: Uuid,
    pub name: String,
    pub layout: String,
    pub is_deck_card: bool,
    pub mana_cost: String,
    pub mana_value: f64,
    pub type_line: String,
    pub oracle_text: String,
    pub keywords: Vec<String>,
    pub colors: i16,
    pub color_identity: i16,
    pub produced_mana: i16,
    pub produces_colorless: bool,
    pub legalities: Json<BTreeMap<String, String>>,
    pub faces: Json<Vec<StoredFace>>,
    pub impl_status: ImplStatus,
}

impl TryFrom<CardRow> for Card {
    type Error = DbError;

    fn try_from(row: CardRow) -> Result<Self, DbError> {
        Ok(Card {
            oracle_id: OracleId(row.oracle_id.to_string()),
            layout: Layout::parse(&row.layout),
            name: row.name,
            is_deck_card: row.is_deck_card,
            mana_cost: row.mana_cost,
            mana_value: row.mana_value,
            type_line: row.type_line,
            oracle_text: row.oracle_text,
            keywords: row.keywords,
            colors: from_mask("cards.colors", row.colors)?,
            color_identity: from_mask("cards.color_identity", row.color_identity)?,
            produced_mana: ProducedMana {
                colors: from_mask("cards.produced_mana", row.produced_mana)?,
                colorless: row.produces_colorless,
            },
            legalities: row
                .legalities
                .0
                .into_iter()
                .map(|(format, keyword)| (format, Legality::parse(&keyword)))
                .collect(),
            faces: row.faces.0,
            impl_status: row.impl_status,
        })
    }
}

/// A `printings` row.
#[derive(Debug)]
pub(crate) struct PrintingRow {
    pub printing_id: Uuid,
    pub oracle_id: Uuid,
    pub set_code: String,
    pub collector_number: String,
    pub rarity: Option<String>,
    pub released_at: Option<NaiveDate>,
    pub lang: String,
    pub digital: bool,
    pub images: Json<PrintingImages>,
}

impl TryFrom<PrintingRow> for Printing {
    type Error = DbError;

    fn try_from(row: PrintingRow) -> Result<Self, DbError> {
        let rarity = row
            .rarity
            .map(|keyword| {
                Rarity::parse(&keyword).ok_or(DbError::Corrupt {
                    column: "printings.rarity",
                    value: keyword,
                })
            })
            .transpose()?;
        Ok(Printing {
            printing_id: PrintingId(row.printing_id.to_string()),
            oracle_id: OracleId(row.oracle_id.to_string()),
            set: row.set_code,
            collector_number: row.collector_number,
            rarity,
            released_at: row.released_at,
            lang: row.lang,
            digital: row.digital,
            images: row.images.0,
        })
    }
}

/// A `rulings` row.
#[derive(Debug)]
pub(crate) struct RulingRow {
    pub oracle_id: Uuid,
    pub source: String,
    pub published_at: Option<NaiveDate>,
    pub comment: String,
}

impl From<RulingRow> for Ruling {
    fn from(row: RulingRow) -> Self {
        Ruling {
            oracle_id: OracleId(row.oracle_id.to_string()),
            source: ruling_source_from_keyword(&row.source),
            published_at: row.published_at,
            comment: row.comment,
        }
    }
}
