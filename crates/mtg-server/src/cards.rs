//! Cards from the card database: the adapter from a stored card row to the strings assembly reads,
//! and the lookup that turns a deck of oracle ids into the definitions the engine plays.
//!
//! A row of `cards` (the imported Scryfall record) gives the printed side; the rules files embedded
//! in `mtg-cards` and the rules the compiler stored in `card_rules` give the abilities (a file wins
//! over a row for the same card); what the engine runs ([`mtg_engine::caps()`]) decides which of
//! those abilities reach the definition (`docs/cards-dsl.md`, "Printed side and rules").

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::assemble::Assembled;
use mtg_cards::deck::{CardFacts, Legality};
use mtg_cards::load::{GeneratedRules, Library};
use mtg_cards::support::SupportRecord;
use mtg_cards::{Caps, CardDefinition, CardIdentity, PrintedCard, PrintedFace, builtin};
use mtg_core::mana::ManaType;
use mtg_db::codec::legality_keyword;
use mtg_db::{Card, Database, DbError, IndexVersion, RulesVersion, StoredFace};
use mtg_import::ids::OracleId;
use mtg_import::resolve::CardIndex;
use sqlx::PgPool;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::images::{Images, Warmer};
use crate::precons::{PreconCache, PreconError, PreconIndex};
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

/// `definition` named as the card `oracle_id`, on the card and on each of its faces, so a view can say which card it is.
pub(crate) fn identified(mut definition: CardDefinition, oracle_id: &str) -> CardDefinition {
    let identity = CardIdentity {
        oracle_id: oracle_id.to_owned(),
        printing_id: None,
    };
    for face in &mut definition.faces {
        face.identity = Some(identity.clone());
    }
    definition.identity = Some(identity);
    definition
}

/// A stored card (the import record as the `cards` table keeps it) as the strings assembly reads.
pub fn printed_card(card: &Card) -> PrintedCard {
    PrintedCard {
        layout: card.layout.keyword().to_owned(),
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

/// The rules and capabilities one request reads cards with, and the version of the stored rules
/// they include: every card of a request is judged under the same rules.
#[derive(Debug, Clone)]
pub(crate) struct Rules {
    library: Arc<Library>,
    caps: Caps,
    version: RulesVersion,
}

impl Rules {
    /// The hand-written and the compiled rules.
    pub(crate) fn library(&self) -> &Library {
        &self.library
    }

    /// What the engine runs.
    pub(crate) fn caps(&self) -> &Caps {
        &self.caps
    }

    /// Which stored rules these are: another version means a row was added, written again or removed.
    pub(crate) fn version(&self) -> &RulesVersion {
        &self.version
    }

    /// How far the game plays a stored card: the pure lookup the deck check calls.
    pub(crate) fn support_of(&self, card: &Card) -> SupportRecord {
        self.library.support(&printed_card(card), &self.caps)
    }
}

/// Where the server gets the definitions of the cards of a deck: the card database, the rules and
/// the capabilities of the engine. Cheap to clone.
#[derive(Debug, Clone)]
pub struct CardSource {
    db: Database,
    rules: Option<Arc<Library>>,
    caps: Caps,
    names: Arc<NameIndex>,
    precons: Arc<PreconCache>,
    stored: Arc<StoredRules>,
    images: Option<Images>,
}

/// The library of the hand-written rules and the rows of `card_rules`, built when the rows change.
#[derive(Debug, Default)]
struct StoredRules {
    built: Mutex<Option<(RulesVersion, Arc<Library>)>>,
}

/// The index that ties the names and printings of a pasted list to cards, built from the stored
/// cards the first time it is needed and again whenever they change.
#[derive(Debug, Default)]
struct NameIndex {
    built: Mutex<Option<(IndexVersion, Arc<CardIndex>)>>,
}

impl CardSource {
    /// A source over the database of `pool` (its `cards` table filled by an import), the rules files
    /// of this build and the capabilities of this engine.
    pub fn new(pool: PgPool) -> CardSource {
        CardSource {
            db: Database::from_pool(pool),
            rules: None,
            caps: mtg_engine::caps(),
            names: Arc::default(),
            precons: Arc::default(),
            stored: Arc::default(),
            images: None,
        }
    }

    /// The same source with the picture cache the card pictures are served from and warmed into.
    #[must_use]
    pub fn with_images(mut self, images: Images) -> CardSource {
        self.images = Some(images);
        self
    }

    /// The picture cache; none until [`CardSource::with_images`] or [`crate::Server::open_with`] gives one.
    pub fn images(&self) -> Option<&Images> {
        self.images.as_ref()
    }

    /// What starts the fetching of a game's pictures, if the source has a picture cache.
    pub(crate) fn warmer(&self) -> Option<Warmer> {
        let images = self.images.clone()?;
        Some(Warmer::new(images, self.db.cards()))
    }

    /// The same source with other hand-written rules than the embedded ones (tests); the stored
    /// compiled rules still join them.
    #[must_use]
    pub fn with_rules(mut self, rules: Library) -> CardSource {
        self.rules = Some(Arc::new(rules));
        // What the library and the index hold was built from the old rules.
        self.stored = Arc::default();
        self.precons = Arc::default();
        self
    }

    /// The same source for an engine with other capabilities.
    #[must_use]
    pub fn with_caps(mut self, caps: Caps) -> CardSource {
        self.caps = caps;
        self.precons = Arc::default();
        self
    }

    /// The hand-written rules files this source assembles with.
    fn hand_written(&self) -> Result<&Library, CatalogError> {
        match &self.rules {
            Some(rules) => Ok(rules),
            None => builtin::library().map_err(|errors| CatalogError::Rules(errors.to_string())),
        }
    }

    /// The rules files and the compiled rules the database holds now: the library is built again
    /// when a row of `card_rules` was added, written again or removed, and a file wins over a row
    /// for the same card. A row this build cannot read is left out and logged.
    async fn library(&self) -> Result<(Arc<Library>, RulesVersion), CatalogError> {
        let repo = self.db.card_rules();
        let database = |error: DbError| CatalogError::Database(error.to_string());
        let version = repo.version().await.map_err(database)?;
        let mut built = self.stored.built.lock().await;
        if let Some((held, library)) = built.as_ref()
            && *held == version
        {
            return Ok((library.clone(), version));
        }
        let rows: Vec<GeneratedRules> = repo
            .all()
            .await
            .map_err(database)?
            .into_iter()
            .map(|row| GeneratedRules {
                oracle_id: row.oracle_id.as_str().to_owned(),
                rules: row.rules,
            })
            .collect();
        let (library, left_out) = self.hand_written()?.with_generated(&rows);
        for error in &left_out {
            tracing::warn!(%error, "compiled rules left out");
        }
        let library = Arc::new(library);
        *built = Some((version.clone(), library.clone()));
        Ok((library, version))
    }

    /// The rules and capabilities to read cards with, as of now.
    pub(crate) async fn rules(&self) -> Result<Rules, CatalogError> {
        let (library, version) = self.library().await?;
        Ok(Rules {
            library,
            caps: self.caps.clone(),
            version,
        })
    }

    /// The card database behind this source.
    pub(crate) fn database(&self) -> &Database {
        &self.db
    }

    /// The index over the deck cards, their printings and the other names of their printings.
    ///
    /// Building it reads every stored printing, so it is kept until the cards or printings change
    /// (a cheap fingerprint tells); requests that arrive while it is built wait for that build.
    pub(crate) async fn card_index(&self) -> Result<Arc<CardIndex>, DbError> {
        let repo = self.db.cards();
        let version = repo.index_version().await?;
        let mut built = self.names.built.lock().await;
        if let Some((held, index)) = built.as_ref()
            && *held == version
        {
            return Ok(index.clone());
        }
        let index = Arc::new(repo.card_index().await?);
        *built = Some((version, index.clone()));
        Ok(index)
    }

    /// The index of every stored precon with its deck check, kept until the precons, the cards or
    /// the printings change (`precons.rs`).
    pub(crate) async fn precon_index(&self) -> Result<Arc<PreconIndex>, PreconError> {
        self.precons.index(self).await
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
        let (rules, _) = self.library().await?;
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
        Ok(Catalog::new(assembled.into_iter().map(|(id, card)| {
            let definition = identified(card.definition, &id);
            (id, Arc::new(definition))
        })))
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

#[cfg(test)]
mod tests {
    //! Goal: a card the server reads for a game names its oracle id on the card and on each face, and keeps its deck fingerprint.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use mtg_cards::definition::Layout;
    use rstest::rstest;

    use super::*;
    use crate::recipe::DeckFingerprint;

    #[rstest]
    #[case::one_face(0)]
    #[case::two_faces(2)]
    fn a_card_and_each_face_name_the_oracle_id(#[case] faces: usize) {
        let mut card = CardDefinition::new("Twin");
        if faces > 0 {
            let each = (0..faces).map(|i| CardDefinition::new(format!("Face {i}")));
            card = card.with_faces(Layout::ModalDfc, each.collect());
        }
        let before = DeckFingerprint::of(&[Arc::new(card.clone())]).unwrap();
        let named = identified(card, "abc");
        let expected = Some(CardIdentity {
            oracle_id: "abc".to_owned(),
            printing_id: None,
        });
        assert_eq!(named.identity, expected);
        assert!(named.faces.iter().all(|face| face.identity == expected));
        assert_eq!(DeckFingerprint::of(&[Arc::new(named)]).unwrap(), before);
    }
}
