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
use mtg_cards::{
    Caps, CardDefinition, CardIdentity, DraftRules, PrintedCard, PrintedFace, ReadState, builtin,
};
use mtg_core::mana::ManaType;
use mtg_db::codec::legality_keyword;
use mtg_db::{
    Card, Database, DbError, DraftKey, DraftRead, DraftsVersion, IndexVersion, RulesVersion,
    SOURCE_FORGE, StoredDraft, StoredFace,
};
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

/// `card` as the printing `printing_id` of its card, on the card and on each face.
fn printed_as(card: &CardDefinition, printing_id: &str) -> CardDefinition {
    let mut printed = card.clone();
    for identity in printed
        .faces
        .iter_mut()
        .map(|face| &mut face.identity)
        .chain([&mut printed.identity])
        .flatten()
    {
        identity.printing_id = Some(printing_id.to_owned());
    }
    printed
}

/// `card` with the printing `printing` on the card and on each face; none clears it.
pub(crate) fn reprinted(card: &CardDefinition, printing: Option<&str>) -> CardDefinition {
    let mut printed = card.clone();
    for identity in printed
        .faces
        .iter_mut()
        .map(|face| &mut face.identity)
        .chain([&mut printed.identity])
        .flatten()
    {
        identity.printing_id = printing.map(str::to_owned);
    }
    printed
}

/// `cards` with the printing each position names, as the deck line that holds the card chose it. Copies of one card in
/// one printing share a definition; a position with no printing, or a card with no identity, keeps its card. The
/// printing is a label for the table's art: the rules, the fingerprint and the game state do not see it.
pub(crate) fn with_printings(
    cards: &[Arc<CardDefinition>],
    printings: &[Option<String>],
) -> Vec<Arc<CardDefinition>> {
    let mut made: BTreeMap<(String, String), Arc<CardDefinition>> = BTreeMap::new();
    cards
        .iter()
        .enumerate()
        .map(|(at, card)| {
            let printing = printings.get(at).and_then(Option::as_deref);
            let (Some(printing), Some(identity)) = (printing, card.identity.as_ref()) else {
                return Arc::clone(card);
            };
            let key = (identity.oracle_id.clone(), printing.to_owned());
            Arc::clone(
                made.entry(key)
                    .or_insert_with(|| Arc::new(printed_as(card, printing))),
            )
        })
        .collect()
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
pub(crate) fn canonical(id: &str) -> String {
    Uuid::parse_str(id).map_or_else(|_| id.to_owned(), |uuid| uuid.to_string())
}

/// The draft `oracle_id` plays on in `library` when the card plays on a draft (`read`: its definition fails soft, which
/// holds for `Read`, `Forge` and a proved partial draft).
pub(crate) fn pin_of(library: &Library, oracle_id: &str, read: bool) -> Option<(String, String)> {
    read.then(|| library.draft(oracle_id))
        .flatten()
        .map(|draft| (oracle_id.to_owned(), draft.read_hash.clone()))
}

/// The rules and capabilities one request reads cards with, and the version of the stored rules
/// they include: every card of a request is judged under the same rules.
#[derive(Debug, Clone)]
pub(crate) struct Rules {
    library: Arc<Library>,
    caps: Caps,
    version: StoredVersion,
}

/// Which stored rules a library was built from: the compiled rows and the drafts with their verdicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredVersion {
    rules: RulesVersion,
    drafts: DraftsVersion,
}

/// The `read_hash` of the draft each card plays on, by oracle id (`GameRecipe::reads`).
pub(crate) type Reads = BTreeMap<String, String>;

impl Rules {
    /// The hand-written and the compiled rules, and the drafts that run on a read.
    pub(crate) fn library(&self) -> &Library {
        &self.library
    }

    /// What the engine runs.
    pub(crate) fn caps(&self) -> &Caps {
        &self.caps
    }

    /// Which stored rules these are: another version means a row was added, written again or removed, or a draft
    /// or a verdict changed.
    pub(crate) fn version(&self) -> &StoredVersion {
        &self.version
    }

    /// How far the game plays a stored card: the pure lookup the deck check calls.
    pub(crate) fn support_of(&self, card: &Card) -> SupportRecord {
        self.library.support(&printed_card(card), &self.caps)
    }

    /// Whether a stored card plays on a draft today: a read (`Read` or `Forge`) or a proved partial draft.
    pub(crate) fn plays_draft(&self, card: &Card) -> bool {
        self.library
            .assemble(&printed_card(card), &self.caps)
            .definition
            .read
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

/// The library of the hand-written rules and the rows of `card_rules`, built when the rows change, and the same with
/// the drafts that run on a read, built again when a draft or a verdict changes.
#[derive(Debug, Default)]
struct StoredRules {
    built: Mutex<Option<Built>>,
}

/// What [`StoredRules`] holds.
#[derive(Debug)]
struct Built {
    rules: RulesVersion,
    /// None when the drafts could not be read, so the next call reads them again.
    drafts: Option<DraftsVersion>,
    /// The hand-written and the compiled rules, without drafts: what a game that pinned no read builds with.
    base: Arc<Library>,
    /// `base` with the current drafts and their read state.
    full: Arc<Library>,
}

/// A stored draft with its read state as a library takes it. A `forge` draft whose Oracle text matched is trusted.
fn draft_rules(draft: StoredDraft, read: mtg_db::ReadState, forge: bool) -> DraftRules {
    DraftRules {
        oracle_id: draft.oracle_id.as_str().to_owned(),
        read_hash: draft.read_hash,
        rules: draft.rules,
        read: match read {
            mtg_db::ReadState::Pending => ReadState::Pending,
            mtg_db::ReadState::Approved => ReadState::Approved,
            mtg_db::ReadState::Wrong => ReadState::Wrong,
        },
        forge,
        // A proof that does not parse is none: a partial draft without one is left out by the library.
        proof: draft
            .proof
            .and_then(|proof| serde_json::from_value(proof).ok()),
    }
}

/// Whether a stored draft is a trusted translation of a Forge script.
fn is_trusted_forge(draft: &StoredDraft) -> bool {
    draft.source == SOURCE_FORGE && draft.oracle_matches
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
    async fn library(&self) -> Result<(Arc<Library>, StoredVersion), CatalogError> {
        let (_, full, version) = self.layers().await?;
        Ok((full, version))
    }

    /// The library without drafts, what a game builds with that pinned no read.
    async fn base_library(&self) -> Result<Arc<Library>, CatalogError> {
        Ok(self.layers().await?.0)
    }

    /// The library without drafts, the same with the current drafts and their read state, and the version of both.
    /// The rows of `card_rules` rebuild both; a draft or a verdict that changed rebuilds only the second.
    async fn layers(&self) -> Result<(Arc<Library>, Arc<Library>, StoredVersion), CatalogError> {
        let repo = self.db.card_rules();
        let database = |error: DbError| CatalogError::Database(error.to_string());
        let rules = repo.version().await.map_err(database)?;
        let drafts = self.draft_version().await;
        let mut built = self.stored.built.lock().await;
        let held = built.take().filter(|held| held.rules == rules);
        let base = match &held {
            Some(held) => held.base.clone(),
            None => Arc::new(self.compiled_library(&repo).await?),
        };
        let full = match held {
            Some(held) if held.drafts.as_ref() == Some(&drafts) => {
                (held.full, Some(drafts.clone()))
            }
            _ => match self.with_current_drafts(&base).await {
                Some(library) => (Arc::new(library), Some(drafts.clone())),
                // Served this once, never cached as current.
                None => (base.clone(), None),
            },
        };
        let (full, cached) = full;
        *built = Some(Built {
            rules: rules.clone(),
            drafts: cached,
            base: base.clone(),
            full: full.clone(),
        });
        Ok((base, full, StoredVersion { rules, drafts }))
    }

    /// The version of the drafts and verdicts; a database that cannot say (the tables are missing) has none, and
    /// the library then holds no drafts.
    async fn draft_version(&self) -> DraftsVersion {
        match self.db.drafts().version().await {
            Ok(version) => version,
            Err(error) => {
                tracing::warn!(%error, "the drafts could not be versioned");
                DraftsVersion::default()
            }
        }
    }

    /// The hand-written rules with the compiled rows.
    async fn compiled_library(
        &self,
        repo: &mtg_db::CardRulesRepo,
    ) -> Result<Library, CatalogError> {
        let rows: Vec<GeneratedRules> = repo
            .all()
            .await
            .map_err(|error| CatalogError::Database(error.to_string()))?
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
        Ok(library)
    }

    /// `base` with the current drafts and the state of their reads; none if they cannot be read.
    async fn with_current_drafts(&self, base: &Library) -> Option<Library> {
        match self.current_drafts().await {
            Ok(rows) => {
                let (library, left_out) = base.with_drafts(&rows);
                for error in &left_out {
                    tracing::warn!(%error, "draft left out");
                }
                Some(library)
            }
            Err(error) => {
                tracing::warn!(%error, "the drafts could not be read");
                None
            }
        }
    }

    /// The current draft of every card with the state its verdicts give it.
    async fn current_drafts(&self) -> Result<Vec<DraftRules>, DbError> {
        let drafts = self.db.drafts().current().await?;
        let states: BTreeMap<String, DraftRead> = self
            .db
            .reads()
            .state()
            .await?
            .into_iter()
            .map(|read| (read.oracle_id.as_str().to_owned(), read))
            .collect();
        Ok(drafts
            .into_iter()
            .map(|draft| {
                let read = states
                    .get(draft.oracle_id.as_str())
                    .filter(|read| read.read_hash == draft.read_hash);
                let state = read.map_or(mtg_db::ReadState::Pending, |read| read.state);
                let forge = read.is_some_and(|read| read.forge);
                draft_rules(draft, state, forge)
            })
            .collect())
    }

    /// The library a game that pinned `reads` builds with: the hand-written and compiled rules and exactly those
    /// drafts, each as read and found correct whatever the verdicts are now.
    async fn pinned_library(&self, reads: &Reads) -> Result<Arc<Library>, CatalogError> {
        let base = self.base_library().await?;
        let keys: Vec<DraftKey> = reads
            .iter()
            .map(|(card, hash)| DraftKey {
                oracle_id: OracleId(canonical(card)),
                read_hash: hash.clone(),
            })
            .collect();
        let stored = self
            .db
            .drafts()
            .get(&keys)
            .await
            .map_err(|error| CatalogError::Database(error.to_string()))?;
        let found: Vec<&str> = stored
            .iter()
            .map(|draft| draft.oracle_id.as_str())
            .collect();
        let missing: Vec<String> = reads
            .keys()
            .filter(|card| !found.contains(&canonical(card).as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(CatalogError::NotFound { ids: missing });
        }
        let rows: Vec<DraftRules> = stored
            .into_iter()
            .map(|draft| {
                let forge = is_trusted_forge(&draft);
                draft_rules(draft, mtg_db::ReadState::Approved, forge)
            })
            .collect();
        let (library, left_out) = base.with_drafts(&rows);
        for error in &left_out {
            tracing::warn!(%error, "pinned draft left out");
        }
        Ok(Arc::new(library))
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
        Ok(self.assemble_pinned(ids).await?.0)
    }

    /// [`CardSource::assemble`] with the draft each card that runs on a read plays on, from the same library.
    pub(crate) async fn assemble_pinned(
        &self,
        ids: &[String],
    ) -> Result<(BTreeMap<String, Assembled>, Reads), CatalogError> {
        let (rules, _) = self.library().await?;
        self.assemble_in(&rules, ids).await
    }

    async fn assemble_in(
        &self,
        rules: &Library,
        ids: &[String],
    ) -> Result<(BTreeMap<String, Assembled>, Reads), CatalogError> {
        let mut wanted: Vec<&String> = Vec::new();
        for id in ids {
            if !wanted.contains(&id) {
                wanted.push(id);
            }
        }
        if wanted.is_empty() {
            return Ok((BTreeMap::new(), Reads::new()));
        }
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
        let mut reads = Reads::new();
        let mut missing = Vec::new();
        for id in wanted {
            match by_id.get(&canonical(id)) {
                Some(card) => {
                    if let Some((card_id, hash)) =
                        pin_of(rules, &canonical(id), card.definition.read)
                    {
                        reads.insert(card_id, hash);
                    }
                    assembled.insert(id.clone(), card.clone());
                }
                None => missing.push(id.clone()),
            }
        }
        if missing.is_empty() {
            Ok((assembled, reads))
        } else {
            Err(CatalogError::NotFound { ids: missing })
        }
    }

    /// The definitions of the distinct cards of `ids`, by id.
    pub async fn catalog(&self, ids: &[String]) -> Result<Catalog, CatalogError> {
        Ok(self.catalog_pinned(ids).await?.0)
    }

    /// [`CardSource::catalog`] with the draft each card that runs on a read plays on.
    pub(crate) async fn catalog_pinned(
        &self,
        ids: &[String],
    ) -> Result<(Catalog, Reads), CatalogError> {
        let (assembled, reads) = self.assemble_pinned(ids).await?;
        Ok((catalog_of(assembled), reads))
    }

    /// The cards of a deck of oracle ids in deck order, copies sharing one definition.
    pub async fn deck(&self, ids: &[String]) -> Result<Vec<Arc<CardDefinition>>, CatalogError> {
        Ok(self.deck_pinned(ids).await?.0)
    }

    /// [`CardSource::deck`] with the draft each card that runs on a read plays on.
    pub(crate) async fn deck_pinned(
        &self,
        ids: &[String],
    ) -> Result<(Vec<Arc<CardDefinition>>, Reads), CatalogError> {
        let (catalog, reads) = self.catalog_pinned(ids).await?;
        let cards = ids
            .iter()
            .filter_map(|id| catalog.get(id).cloned())
            .collect();
        Ok((cards, reads))
    }

    /// The definitions a recipe needs, built as the game was created: a recipe that pinned reads gets exactly those
    /// drafts as approved, one that pinned none gets no draft. A recipe with no deck of oracle ids needs none and
    /// does not touch the database, so games with demo decks resume on a server whose database has no cards.
    pub async fn catalog_for(&self, recipe: &GameRecipe) -> Result<Catalog, CatalogError> {
        let ids = recipe.oracle_ids();
        if ids.is_empty() {
            return Ok(Catalog::default());
        }
        let rules = if recipe.reads.is_empty() {
            self.base_library().await?
        } else {
            self.pinned_library(&recipe.reads).await?
        };
        let (assembled, _) = self.assemble_in(&rules, &ids).await?;
        Ok(catalog_of(assembled))
    }
}

/// The definitions of assembled cards, each named as the card it is.
fn catalog_of(assembled: BTreeMap<String, Assembled>) -> Catalog {
    Catalog::new(assembled.into_iter().map(|(id, card)| {
        let definition = identified(card.definition, &id);
        (id, Arc::new(definition))
    }))
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

    fn named(oracle_id: &str, faces: usize) -> Arc<CardDefinition> {
        let mut card = CardDefinition::new(oracle_id);
        if faces > 0 {
            let each = (0..faces).map(|i| CardDefinition::new(format!("Face {i}")));
            card = card.with_faces(Layout::ModalDfc, each.collect());
        }
        Arc::new(identified(card, oracle_id))
    }

    fn printing(card: &CardDefinition) -> Option<&str> {
        card.identity.as_ref()?.printing_id.as_deref()
    }

    #[rstest]
    #[case::none_named(&[], &[None, None, None])]
    #[case::all_none(&[None, None, None], &[None, None, None])]
    #[case::one_printing_for_the_copies(&[Some("p"), Some("p"), None], &[Some("p"), Some("p"), None])]
    #[case::two_printings_of_one_card(&[Some("p"), Some("q"), Some("p")], &[Some("p"), Some("q"), Some("p")])]
    #[case::a_short_list_leaves_the_rest(&[Some("p")], &[Some("p"), None, None])]
    fn a_position_takes_the_printing_its_line_chose(
        #[case] asked: &[Option<&str>],
        #[case] wanted: &[Option<&str>],
        #[values(0, 2)] faces: usize,
    ) {
        let cards = vec![named("a", faces); 3];
        let asked: Vec<Option<String>> = asked.iter().map(|p| p.map(str::to_owned)).collect();
        let printed = with_printings(&cards, &asked);
        let got: Vec<Option<&str>> = printed.iter().map(|card| printing(card)).collect();
        assert_eq!(got, wanted);
        for (card, wanted) in printed.iter().zip(wanted) {
            let identity = card.identity.as_ref().unwrap();
            assert_eq!(identity.oracle_id, "a");
            assert!(card.faces.iter().all(|face| face.identity == card.identity));
            assert_eq!(identity.printing_id.as_deref(), *wanted);
        }
        // Copies in one printing share a definition, and the fingerprint does not see the printing.
        assert!(Arc::ptr_eq(&printed[0], &printed[0]));
        if wanted[0].is_some() && wanted[0] == wanted[2] {
            assert!(Arc::ptr_eq(&printed[0], &printed[2]));
        }
        assert_eq!(
            DeckFingerprint::of(&printed).unwrap(),
            DeckFingerprint::of(&cards).unwrap()
        );
    }

    #[test]
    fn a_card_with_no_identity_keeps_to_itself() {
        let bare = Arc::new(CardDefinition::new("Bare"));
        let printed = with_printings(std::slice::from_ref(&bare), &[Some("p".to_owned())]);
        assert!(Arc::ptr_eq(&printed[0], &bare));
        assert_eq!(printed[0].identity, None);
    }

    const DRAFTED: &str = "4457ed35-7c10-48c8-9776-456485fdf070";

    fn library_with_draft() -> Library {
        let card = mtg_cards::card::Card {
            oracle_id: DRAFTED.into(),
            name: "Vanilla".into(),
            impl_status: mtg_cards::card::ImplStatus::Full,
            faces: vec![mtg_cards::card::Face {
                name: "Vanilla".into(),
                oracle_fingerprint: mtg_cards::support::oracle_fingerprint(""),
                abilities: vec![],
            }],
            notes: None,
        };
        let row = DraftRules {
            oracle_id: DRAFTED.into(),
            read_hash: "h1".into(),
            rules: serde_json::to_value(card).unwrap(),
            read: ReadState::Approved,
            forge: true,
            proof: None,
        };
        let (library, left_out) = Library::default().with_drafts(&[row]);
        assert!(left_out.is_empty(), "{left_out:?}");
        library
    }

    #[rstest]
    #[case::plays_on_a_draft(DRAFTED, true, Some("h1"))]
    #[case::plays_on_no_draft(DRAFTED, false, None)]
    #[case::has_no_draft("other", true, None)]
    fn a_card_pins_the_draft_it_plays_on_whatever_its_state(
        #[case] card: &str,
        #[case] read: bool,
        #[case] pinned: Option<&str>,
    ) {
        let library = library_with_draft();

        let pin = pin_of(&library, card, read);

        assert_eq!(pin.map(|(_, hash)| hash).as_deref(), pinned);
    }

    const LINE0: &str = "Lightning Bolt deals 3 damage to any target.";
    const LINE1: &str = "Whenever you cast a spell, scry 1.";

    fn stored_partial(proof_end: usize) -> StoredDraft {
        let text = format!("{LINE0}\n{LINE1}");
        let (start, end) = (LINE0.chars().count() + 1, text.chars().count());
        let bolt = r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#;
        let rules = serde_json::json!({
            "oracle_id": DRAFTED, "name": "Lightning Bolt", "impl_status": "Partial",
            "faces": [{"name": "Lightning Bolt", "oracle_fingerprint": mtg_cards::support::oracle_fingerprint(&text),
                "abilities": [
                    serde_json::from_str::<serde_json::Value>(bolt).unwrap(),
                    {"Manual": {"reason": "by hand", "span": {"start": start, "end": end}}}]}]
        });
        let mark = |start: usize, end: usize, mark: &str| serde_json::json!({"face": 0, "span": {"start": start, "end": end}, "mark": mark});
        StoredDraft {
            oracle_id: OracleId(DRAFTED.to_owned()),
            read_hash: mtg_cards::support::read_hash(
                &partial_card(),
                &serde_json::from_value(rules.clone()).unwrap(),
            ),
            rules,
            source: SOURCE_FORGE.into(),
            oracle_matches: true,
            proof: Some(serde_json::json!({"marks": [
                mark(0, LINE0.chars().count(), "Auto"), mark(start, proof_end, "ByHand")]})),
            retired: false,
        }
    }

    fn partial_card() -> mtg_cards::PrintedCard {
        mtg_cards::PrintedCard {
            layout: String::new(),
            oracle_id: DRAFTED.into(),
            name: "Lightning Bolt".into(),
            keywords: vec![],
            faces: vec![mtg_cards::PrintedFace {
                name: "Lightning Bolt".into(),
                oracle_text: format!("{LINE0}\n{LINE1}"),
                ..mtg_cards::PrintedFace::default()
            }],
        }
    }

    #[test]
    fn a_stored_proved_partial_draft_plays_as_partial_and_a_tampered_proof_is_left_out() {
        let end = LINE0.chars().count() + 1 + LINE1.chars().count();
        let row = draft_rules(stored_partial(end), mtg_db::ReadState::Approved, true);
        let (library, left_out) = Library::default().with_drafts(&[row]);
        assert!(left_out.is_empty(), "{left_out:?}");
        let assembled = library.assemble(&partial_card(), &mtg_cards::Caps::everything());
        assert_eq!(
            assembled.support.support,
            mtg_cards::support::Support::Partial
        );
        assert!(assembled.definition.read);
        assert_eq!(
            assembled.definition.manual.len(),
            1,
            "the by-hand line stays"
        );

        let tampered = draft_rules(stored_partial(end - 1), mtg_db::ReadState::Approved, true);
        let (library, left_out) = Library::default().with_drafts(&[tampered]);
        assert_eq!(left_out.len(), 1, "{left_out:?}");
        assert!(library.draft(DRAFTED).is_none());
    }

    #[test]
    fn a_partial_draft_stored_without_a_proof_is_left_out() {
        let mut old = stored_partial(0);
        old.proof = None;
        let row = draft_rules(old, mtg_db::ReadState::Approved, true);
        let (library, left_out) = Library::default().with_drafts(&[row]);
        assert_eq!(left_out.len(), 1, "{left_out:?}");
        assert!(library.draft(DRAFTED).is_none());
    }
}
