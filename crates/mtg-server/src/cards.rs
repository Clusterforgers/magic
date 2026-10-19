//! Cards from the card database: the adapter from a stored card row to the strings assembly reads,
//! and the lookup that turns a deck of oracle ids into the definitions the engine plays.
//!
//! A row of `cards` (the imported Scryfall record) gives the printed side; the rules files embedded
//! in `mtg-cards` give the abilities; what the engine runs ([`mtg_engine::caps()`]) decides which of
//! those abilities reach the definition (`docs/cards-dsl.md`, "Printed side and rules").

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::assemble::Assembled;
use mtg_cards::deck::{CardFacts, Legality};
use mtg_cards::load::Library;
use mtg_cards::support::SupportRecord;
use mtg_cards::{Caps, CardDefinition, PrintedCard, PrintedFace, builtin};
use mtg_core::mana::ManaType;
use mtg_db::codec::legality_keyword;
use mtg_db::{Card, Database, StoredFace};
use mtg_import::ids::OracleId;
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::recipe::{Catalog, GameRecipe};

/// Why the cards of a deck could not be had.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CatalogError {
    /// The database holds no card for these oracle ids (or they are not oracle ids at all).
    #[error("no card for the oracle id(s) {}", ids.join(", "))]
    NotFound {
        /// Every id with no card, once each, in the order the deck names them.
        ids: Vec<String>,
    },
    /// The rules files embedded in the server do not load.
    #[error("the card rules do not load: {0}")]
    Rules(String),
    /// The card database could not be read.
    #[error("the card database failed: {0}")]
    Database(String),
}

/// A stored face as the strings assembly reads.
fn printed_face(face: &StoredFace) -> PrintedFace {
    PrintedFace {
        name: face.name.clone(),
        mana_cost: face.mana_cost.clone(),
        type_line: face.type_line.clone(),
        oracle_text: face.oracle_text.clone(),
        power: face.power.clone(),
        toughness: face.toughness.clone(),
        loyalty: face.loyalty.clone(),
        defense: face.defense.clone(),
        colors: face
            .colors
            .iter()
            .map(|color| ManaType::from(color).symbol().to_string())
            .collect(),
    }
}

/// A stored card (the import record as the `cards` table keeps it) as the strings assembly reads.
pub fn printed_card(card: &Card) -> PrintedCard {
    PrintedCard {
        oracle_id: card.oracle_id.to_string(),
        name: card.name.clone(),
        keywords: card.keywords.clone(),
        faces: card.faces.iter().map(printed_face).collect(),
    }
}

/// A stored card as the deck check reads it: the imported side, the color identity and the legalities.
pub(crate) fn card_facts(card: &Card) -> CardFacts {
    CardFacts {
        printed: printed_card(card),
        color_identity: card.color_identity,
        legalities: card
            .legalities
            .iter()
            .map(|(format, legality)| {
                (format.clone(), Legality::parse(legality_keyword(*legality)))
            })
            .collect(),
    }
}

/// The spelling of an oracle id the database answers with, so `A1B2...` and `a1b2...` are one card.
fn canonical(id: &str) -> String {
    Uuid::parse_str(id).map_or_else(|_| id.to_owned(), |uuid| uuid.to_string())
}

/// Where the server gets the definitions of the cards of a deck: the card database, the rules and
/// the capabilities of the engine. Cheap to clone.
#[derive(Debug, Clone)]
pub struct CardSource {
    db: Database,
    rules: Option<Arc<Library>>,
    caps: Caps,
}

impl CardSource {
    /// A source over the database of `pool` (its `cards` table filled by an import), the rules files
    /// of this build and the capabilities of this engine.
    pub fn new(pool: PgPool) -> CardSource {
        CardSource {
            db: Database::from_pool(pool),
            rules: None,
            caps: mtg_engine::caps(),
        }
    }

    /// The same source with other rules than the embedded ones (tests, and a future rules table).
    #[must_use]
    pub fn with_rules(mut self, rules: Library) -> CardSource {
        self.rules = Some(Arc::new(rules));
        self
    }

    /// The same source for an engine with other capabilities.
    #[must_use]
    pub fn with_caps(mut self, caps: Caps) -> CardSource {
        self.caps = caps;
        self
    }

    /// The rules files this source assembles with.
    pub(crate) fn library(&self) -> Result<&Library, CatalogError> {
        match &self.rules {
            Some(rules) => Ok(rules),
            None => builtin::library().map_err(|errors| CatalogError::Rules(errors.to_string())),
        }
    }

    /// What the engine of this source runs.
    pub(crate) fn caps(&self) -> &Caps {
        &self.caps
    }

    /// The card database behind this source.
    pub(crate) fn database(&self) -> &Database {
        &self.db
    }

    /// How far the game plays a stored card: the pure lookup the deck check calls.
    pub(crate) fn support_of(&self, card: &Card) -> Result<SupportRecord, CatalogError> {
        Ok(self.library()?.support(&printed_card(card), &self.caps))
    }

    /// Assembles each distinct card of `ids`, keyed by the id as it was asked for. Every id the
    /// database does not know is named in the error.
    pub async fn assemble(
        &self,
        ids: &[String],
    ) -> Result<BTreeMap<String, Assembled>, CatalogError> {
        let mut wanted: Vec<&String> = Vec::new();
        for id in ids {
            if !wanted.contains(&id) {
                wanted.push(id);
            }
        }
        if wanted.is_empty() {
            return Ok(BTreeMap::new());
        }
        let rules = self.library()?;
        let asked: Vec<OracleId> = wanted.iter().map(|id| OracleId(canonical(id))).collect();
        let found = self
            .db
            .cards()
            .get_many(&asked)
            .await
            .map_err(|error| CatalogError::Database(error.to_string()))?;
        let by_id: BTreeMap<String, Assembled> = found
            .iter()
            .map(|card| {
                let printed = printed_card(card);
                (
                    canonical(card.oracle_id.as_str()),
                    rules.assemble(&printed, &self.caps),
                )
            })
            .collect();
        let mut assembled = BTreeMap::new();
        let mut missing = Vec::new();
        for id in wanted {
            match by_id.get(&canonical(id)) {
                Some(card) => {
                    assembled.insert(id.clone(), card.clone());
                }
                None => missing.push(id.clone()),
            }
        }
        if missing.is_empty() {
            Ok(assembled)
        } else {
            Err(CatalogError::NotFound { ids: missing })
        }
    }

    /// The definitions of the distinct cards of `ids`, by id.
    pub async fn catalog(&self, ids: &[String]) -> Result<Catalog, CatalogError> {
        let assembled = self.assemble(ids).await?;
        Ok(Catalog::new(
            assembled
                .into_iter()
                .map(|(id, card)| (id, Arc::new(card.definition))),
        ))
    }

    /// The cards of a deck of oracle ids in deck order, copies sharing one definition.
    pub async fn deck(&self, ids: &[String]) -> Result<Vec<Arc<CardDefinition>>, CatalogError> {
        let catalog = self.catalog(ids).await?;
        Ok(ids
            .iter()
            .filter_map(|id| catalog.get(id).cloned())
            .collect())
    }

    /// The definitions a recipe needs. A recipe with no deck of oracle ids needs none and does not
    /// touch the database, so games with demo decks resume on a server whose database has no cards.
    pub async fn catalog_for(&self, recipe: &GameRecipe) -> Result<Catalog, CatalogError> {
        let ids = recipe.oracle_ids();
        if ids.is_empty() {
            return Ok(Catalog::default());
        }
        self.catalog(&ids).await
    }
}
