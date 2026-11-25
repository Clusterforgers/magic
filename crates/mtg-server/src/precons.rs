//! The precon decks as the server judges them: each one's deck check and whether a game takes it,
//! the unique code the API names it by, and the index of every precon that the list and the code
//! lookups share.
//!
//! One function judges a precon ([`Judge::judge`]), so the list, one precon and a seat that takes
//! it as its deck agree. The index is built from every stored precon and its cards and kept until
//! the precons, the cards, the printings or the compiled rules change; the rules files and the
//! engine's capabilities are fixed for the life of a [`CardSource`].

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use mtg_cards::deck::{
    CardFacts, Counts, DeckLine as CheckLine, DeckReport, DeckState, DeckZone, Issue, IssueCode,
    ZoneCounts, deck_report,
};
use mtg_cards::load::Library;
use mtg_cards::support::PrintedCard;
use mtg_cards::{Caps, CardDefinition};
use mtg_core::color::ColorSet;
use mtg_core::house::HouseRules;
use mtg_db::{DbError, DeckBoard, IndexVersion, PreconDeckDetail, PreconDeckId, PreconVersion};
use mtg_protocol::rest::{DeckPicture, MAX_DECK_PICTURES, PreconListItem};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::watch;

use crate::card_api::card_colors;
use crate::cards::{
    CardSource, CatalogError, Reads, Rules, StoredVersion, card_facts, identified, off_runtime,
    pin_of, printed_card,
};
use crate::deck_cards::Line;
use crate::deck_choice::{commander_ids, format_issues, library_ids};
use crate::recipe::FormatKind;

/// Longest code, in characters; a longer name is cut.
const MAX_CODE_CHARS: usize = 80;

/// Why the precons could not be had.
#[derive(Debug, Error)]
pub(crate) enum PreconError {
    /// The database failed.
    #[error("the precons could not be read: {0}")]
    Db(#[from] DbError),
    /// The rules files do not load.
    #[error("the precons could not be judged: {0}")]
    Catalog(#[from] CatalogError),
}

/// The board a precon entry sits on, as a zone of the deck check.
pub(crate) fn zone_of(board: DeckBoard) -> DeckZone {
    match board {
        DeckBoard::Commander => DeckZone::Commander,
        DeckBoard::Main => DeckZone::Main,
        DeckBoard::Sideboard => DeckZone::Sideboard,
    }
}

/// An entry of a precon that matches no stored card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Unresolved {
    pub(crate) zone: DeckZone,
    pub(crate) name: String,
    pub(crate) count: u32,
}

/// What the server makes of one precon.
#[derive(Debug)]
pub(crate) struct Judged {
    /// The format the deck check judged it in.
    pub(crate) format: &'static str,
    /// The deck check, entries without a card included.
    pub(crate) report: DeckReport,
    /// The warnings: the legality issues and the rules of the game's format the deck breaks.
    pub(crate) issues: Vec<Issue>,
    /// Whether a game can start with the deck: it has a card the database knows.
    pub(crate) playable: bool,
    /// The lines of the entries that have a card, in deck order.
    pub(crate) lines: Vec<Line>,
    /// The oracle id of every card of the main board in deck order: the library a game is given.
    pub(crate) library: Vec<String>,
    /// Their definitions, in the same order.
    pub(crate) cards: Vec<Arc<CardDefinition>>,
    /// The oracle id of every card of the commander board: the commanders a game starts with.
    pub(crate) commanders: Vec<String>,
    /// Their definitions, in the same order.
    pub(crate) leaders: Vec<Arc<CardDefinition>>,
    /// The draft each card that runs on a read plays on, by oracle id.
    pub(crate) reads: Reads,
}

/// The cards whose pictures stand for a deck in a list: its commanders, else its first card of the main board.
fn pictures_of(deck: &PreconDeckDetail) -> Vec<DeckPicture> {
    let cards = |board: DeckBoard| {
        deck.entries
            .iter()
            .filter(move |entry| entry.board == board)
            .filter_map(|entry| entry.card.as_ref().map(|card| (entry, card)))
            .map(|(entry, card)| DeckPicture {
                oracle_id: card.oracle_id.to_string(),
                name: card.name.clone(),
                printing_id: entry.printing_id.as_ref().map(ToString::to_string),
            })
    };
    let leaders: Vec<DeckPicture> = cards(DeckBoard::Commander)
        .take(MAX_DECK_PICTURES)
        .collect();
    if leaders.is_empty() {
        cards(DeckBoard::Main).take(1).collect()
    } else {
        leaders
    }
}

impl Judged {
    /// The precon as the list shows it, under `code`.
    pub(crate) fn item(&self, deck: &PreconDeckDetail, code: &str) -> PreconListItem {
        let summary = &deck.summary;
        let commanders = deck
            .entries
            .iter()
            .filter(|entry| entry.board == DeckBoard::Commander)
            .map(|entry| {
                entry
                    .card
                    .as_ref()
                    .map_or_else(|| entry.card_name.clone(), |card| card.name.clone())
            })
            .collect();
        let pictures = pictures_of(deck);
        let identity: ColorSet = deck
            .entries
            .iter()
            .filter(|entry| entry.board == DeckBoard::Commander)
            .filter_map(|entry| entry.card.as_ref())
            .flat_map(|card| card.color_identity.iter())
            .collect();
        let card_count = deck
            .entries
            .iter()
            .filter(|entry| entry.board != DeckBoard::Sideboard)
            .map(|entry| entry.count)
            .fold(0, u32::saturating_add);
        let state = self.report.summary.state;
        PreconListItem {
            code: code.to_owned(),
            name: summary.name.clone(),
            set: summary.code.clone(),
            deck_type: summary.deck_type.clone(),
            release_date: summary.release_date.map(|date| date.to_string()),
            commanders,
            pictures,
            color_identity: card_colors(identity),
            card_count,
            format: self.format.to_owned(),
            state,
            verified: state == DeckState::Verified,
            playable: self.playable,
            summary: self.report.summary.clone(),
        }
    }
}

/// Judges precons against one set of rules; remembers what it learned of each card, so judging every
/// precon assembles every card once.
pub(crate) struct Judge<'a> {
    library: &'a Library,
    caps: &'a Caps,
    facts: BTreeMap<String, CardFacts>,
    definitions: HashMap<String, Arc<CardDefinition>>,
    reads: Reads,
}

impl<'a> Judge<'a> {
    /// A judge with the rules and capabilities a request reads cards with.
    pub(crate) fn new(rules: &'a Rules) -> Judge<'a> {
        Judge {
            library: rules.library(),
            caps: rules.caps(),
            facts: BTreeMap::new(),
            definitions: HashMap::new(),
            reads: Reads::new(),
        }
    }

    /// The deck check of a precon in its own format and the engine's rules for the game it is for
    /// on top.
    ///
    /// A deck with a commander is judged as `commander` and is for a Commander game, any other as
    /// `freeform` for a Normal game: a precon is a product, not an entry in a tournament format.
    pub(crate) fn judge(&mut self, deck: &PreconDeckDetail) -> Judged {
        let mut lines = Vec::new();
        let mut unresolved = Vec::new();
        for entry in &deck.entries {
            let zone = zone_of(entry.board);
            match &entry.card {
                Some(card) => {
                    let id = card.oracle_id.to_string();
                    if !self.facts.contains_key(&id) {
                        let printed = printed_card(card);
                        let assembled = self.library.assemble(&printed, self.caps);
                        if let Some((card, hash)) =
                            pin_of(self.library, &id, assembled.definition.read)
                        {
                            self.reads.insert(card, hash);
                        }
                        self.facts.insert(id.clone(), card_facts(card));
                        let definition = identified(assembled.definition, &id);
                        self.definitions.insert(id.clone(), Arc::new(definition));
                    }
                    lines.push(Line {
                        zone,
                        oracle_id: id,
                        printing_id: entry.printing_id.as_ref().map(ToString::to_string),
                        count: entry.count,
                        by_hand: false,
                    });
                }
                None => unresolved.push(Unresolved {
                    zone,
                    name: entry.card_name.clone(),
                    count: entry.count,
                }),
            }
        }
        let commanded = deck
            .entries
            .iter()
            .any(|entry| entry.board == DeckBoard::Commander)
            || deck
                .summary
                .deck_type
                .as_deref()
                .is_some_and(|kind| kind.eq_ignore_ascii_case("Commander Deck"));
        let format = if commanded { "commander" } else { "freeform" };
        let check: Vec<CheckLine> = lines.iter().map(Line::to_check).collect();
        let mut report = deck_report(format, &check, &self.facts, self.library, self.caps);
        count_unresolved(&mut report, &unresolved);

        let library = library_ids(&lines);
        let cards: Vec<Arc<CardDefinition>> = library
            .iter()
            .filter_map(|id| self.definitions.get(id).cloned())
            .collect();
        let commanders = commander_ids(&lines);
        let leaders: Vec<Arc<CardDefinition>> = commanders
            .iter()
            .filter_map(|id| self.definitions.get(id).cloned())
            .collect();
        let kind = if commanded {
            FormatKind::Commander
        } else {
            FormatKind::Normal
        };
        // The engine's check is the second line, for a deck its own check finds legal. Both only warn.
        let mut issues = report.legality.clone();
        if issues.is_empty() {
            issues = format_issues(kind, &cards, &leaders, None, &HouseRules::default());
        }
        let playable = !(cards.is_empty() && leaders.is_empty());
        let reads: Reads = self
            .reads
            .iter()
            .filter(|(card, _)| lines.iter().any(|line| &line.oracle_id == *card))
            .map(|(card, hash)| (card.clone(), hash.clone()))
            .collect();
        Judged {
            format,
            report,
            issues,
            playable,
            lines,
            library,
            cards,
            commanders,
            leaders,
            reads,
        }
    }
}

/// Judges `deck` under `rules` on the blocking pool, so the runtime keeps serving meanwhile.
pub(crate) async fn judged_off(
    rules: &Rules,
    deck: PreconDeckDetail,
) -> Result<(PreconDeckDetail, Judged), PreconError> {
    let rules = rules.clone();
    Ok(off_runtime(move || {
        let judged = Judge::new(&rules).judge(&deck);
        (deck, judged)
    })
    .await?)
}

/// Adds the entries that match no card to a report: an `unknown_card` issue each, their copies as
/// `unplayable`, played by hand, and an illegal deck.
fn count_unresolved(report: &mut DeckReport, unresolved: &[Unresolved]) {
    if unresolved.is_empty() {
        return;
    }
    for entry in unresolved {
        report.legality.push(Issue {
            code: IssueCode::UnknownCard,
            message: format!("{} is not in the card database", entry.name),
            zone: Some(entry.zone),
            oracle_id: None,
            name: Some(entry.name.clone()),
        });
        let summary = &mut report.summary;
        let add = |counts: &mut Counts| {
            counts.total = counts.total.saturating_add(entry.count);
            counts.unplayable = counts.unplayable.saturating_add(entry.count);
        };
        add(&mut summary.counts);
        if let Some(zone) = summary
            .zones
            .iter_mut()
            .find(|zone| zone.zone == entry.zone)
        {
            add(&mut zone.counts);
        } else {
            let mut counts = Counts::default();
            add(&mut counts);
            summary.zones.push(ZoneCounts {
                zone: entry.zone,
                counts,
            });
        }
    }
    report.summary.zones.sort_by_key(|zone| {
        DeckZone::ALL
            .iter()
            .position(|candidate| *candidate == zone.zone)
    });
    report.summary.issues = u32::try_from(report.legality.len()).unwrap_or(u32::MAX);
    report.summary.legal = false;
    report.summary.state = DeckState::of(&report.summary.counts);
}

/// The lower-case words of `text` joined by hyphens: ASCII letters and digits only, apostrophes
/// dropped, every other run of characters one hyphen.
fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in text.chars() {
        if c == '\'' || c == '\u{2019}' {
            continue;
        }
        if c.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.push(c.to_ascii_lowercase());
        } else {
            gap = true;
        }
    }
    out
}

/// One code per deck, in the order given: the set code and the name's slug, cut at
/// [`MAX_CODE_CHARS`]; a code another deck already has ends in the deck's id.
pub(crate) fn codes(decks: &[(PreconDeckId, &str, &str)]) -> Vec<String> {
    let mut taken: HashSet<String> = HashSet::new();
    decks
        .iter()
        .map(|(id, set, name)| {
            let mut base = [slug(set), slug(name)]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("-");
            base.truncate(MAX_CODE_CHARS);
            let base = base.trim_end_matches('-').to_owned();
            let mut code = if base.is_empty() {
                "precon".to_owned()
            } else {
                base.clone()
            };
            let mut next = 0u32;
            while taken.contains(&code) {
                code = if next == 0 {
                    format!("{base}-{}", id.0)
                } else {
                    format!("{base}-{}-{next}", id.0)
                };
                next += 1;
            }
            taken.insert(code.clone());
            code
        })
        .collect()
}

/// One precon of the index as `precon_judgements` keeps it.
#[derive(Serialize, Deserialize)]
struct KeptDeck<'a> {
    id: i64,
    item: Cow<'a, PreconListItem>,
}

/// One precon of the index.
#[derive(Debug)]
pub(crate) struct Indexed {
    /// The stored deck.
    pub(crate) id: PreconDeckId,
    /// The deck as the list shows it.
    pub(crate) item: PreconListItem,
}

/// Every stored precon with its judgment, in the order of the list.
#[derive(Debug, Default)]
pub(crate) struct PreconIndex {
    decks: Vec<Indexed>,
    by_code: HashMap<String, usize>,
}

impl PreconIndex {
    /// The precons, newest release first, then by name.
    pub(crate) fn items(&self) -> impl Iterator<Item = &PreconListItem> {
        self.decks.iter().map(|indexed| &indexed.item)
    }

    /// The precon with this code.
    pub(crate) fn find(&self, code: &str) -> Option<&Indexed> {
        self.by_code.get(code).and_then(|at| self.decks.get(*at))
    }

    /// The index as `precon_judgements` keeps it: its precons as JSON, in list order.
    fn to_kept(&self) -> Result<String, serde_json::Error> {
        let kept: Vec<KeptDeck<'_>> = self
            .decks
            .iter()
            .map(|indexed| KeptDeck {
                id: indexed.id.0,
                item: Cow::Borrowed(&indexed.item),
            })
            .collect();
        serde_json::to_string(&kept)
    }

    /// An index read back from [`PreconIndex::to_kept`].
    fn from_kept(kept: &str) -> Result<PreconIndex, serde_json::Error> {
        let kept: Vec<KeptDeck<'_>> = serde_json::from_str(kept)?;
        Ok(PreconIndex::of(
            kept.into_iter()
                .map(|deck| Indexed {
                    id: PreconDeckId(deck.id),
                    item: deck.item.into_owned(),
                })
                .collect(),
        ))
    }

    /// The index of `decks`, in the order given.
    fn of(decks: Vec<Indexed>) -> PreconIndex {
        let by_code = decks
            .iter()
            .enumerate()
            .map(|(at, indexed)| (indexed.item.code.clone(), at))
            .collect();
        PreconIndex { decks, by_code }
    }

    fn build(rules: &Rules, details: &[PreconDeckDetail]) -> PreconIndex {
        let named: Vec<(PreconDeckId, &str, &str)> = details
            .iter()
            .map(|deck| {
                (
                    deck.summary.id,
                    deck.summary.code.as_str(),
                    deck.summary.name.as_str(),
                )
            })
            .collect();
        let codes = codes(&named);
        decide_all(rules, details);
        let mut judge = Judge::new(rules);
        let decks: Vec<Indexed> = details
            .iter()
            .zip(&codes)
            .map(|(deck, code)| Indexed {
                id: deck.summary.id,
                item: judge.judge(deck).item(deck, code),
            })
            .collect();
        PreconIndex::of(decks)
    }
}

/// Most threads [`decide_cards`] runs on.
pub(crate) const DECIDING_THREADS: usize = 8;

/// Decides every card of the precons once, spread over a few threads, so the library holds the decisions before the
/// decks are judged one by one: the first build decides thousands of cards, each in milliseconds.
fn decide_all(rules: &Rules, details: &[PreconDeckDetail]) {
    let cards: Vec<PrintedCard> = details
        .iter()
        .flat_map(|deck| deck.entries.iter())
        .filter_map(|entry| entry.card.as_ref())
        .map(|card| (card.oracle_id.to_string(), card))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .map(printed_card)
        .collect();
    decide_cards(rules, &cards);
}

/// Decides `cards` once, spread over a few threads, so the library holds their decisions before a deck check asks.
pub(crate) fn decide_cards(rules: &Rules, cards: &[PrintedCard]) {
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .clamp(1, DECIDING_THREADS);
    let (library, caps) = (rules.library(), rules.caps());
    std::thread::scope(|scope| {
        for part in cards.chunks(cards.len().div_ceil(threads).max(1)) {
            scope.spawn(move || {
                for card in part {
                    library.assemble(card, caps);
                    library.draft_state(card, caps);
                }
            });
        }
    });
}

/// What the stored precons, cards and compiled rules looked like when an index was built.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    cards: IndexVersion,
    precons: PreconVersion,
    rules: StoredVersion,
}

impl Stamp {
    /// The stamp of what the database holds now, read without building the library.
    async fn now(source: &CardSource) -> Result<Stamp, PreconError> {
        let database = source.database();
        Ok(Stamp {
            cards: database.cards().index_version().await?,
            precons: database.precons().version().await?,
            rules: source.stored_version().await?,
        })
    }

    /// The fingerprint an index of this stamp is kept under in `precon_judgements`: the stamp, the server build and
    /// the engine's capabilities; none when `source` keeps no judgements.
    fn fingerprint(&self, source: &CardSource) -> Option<String> {
        let under = source.kept_under()?;
        let digest = Sha256::digest(format!("{under}|{self:?}").as_bytes());
        Some(digest.iter().fold(String::new(), |mut out, byte| {
            // Writing to a String does not fail.
            let _ = write!(out, "{byte:02x}");
            out
        }))
    }
}

/// How long a request waits for a build of the index before it is served the index held from before.
pub(crate) const FRESH_WAIT: Duration = Duration::from_millis(500);

/// A build of the index that every request of the same stamp shares; the error is its message.
type Building = watch::Receiver<Option<Result<Arc<PreconIndex>, String>>>;

/// What [`PreconCache`] holds: the index last built and the build under way.
#[derive(Default)]
struct Slots {
    built: Option<(Stamp, Arc<PreconIndex>)>,
    building: Option<(Stamp, Building)>,
}

/// The index kept in `precon_judgements` when the process started, with its fingerprint.
type Kept = Option<(String, Arc<PreconIndex>)>;

/// The index of the precons, built the first time it is needed and again when the precons, the
/// cards, the printings or the stored rules change, and kept in the database across starts.
///
/// One build runs at a time per stamp, in a task of its own, its judging on the blocking pool. A request waits for it
/// at most [`FRESH_WAIT`] while an older index is held and is then served the older one, so no request queues behind
/// a whole build once the first one is done. The index kept from before the start counts as held: of the current
/// fingerprint it is the current index, of another it is served while the current one is built.
#[derive(Default)]
pub(crate) struct PreconCache {
    slots: Arc<std::sync::Mutex<Slots>>,
    kept: tokio::sync::OnceCell<Kept>,
}

impl std::fmt::Debug for PreconCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreconCache")
    }
}

/// The slots, whatever a panic left in them: each write leaves them whole.
fn slots(slots: &std::sync::Mutex<Slots>) -> std::sync::MutexGuard<'_, Slots> {
    slots.lock().unwrap_or_else(PoisonError::into_inner)
}

/// An index as a request is served it.
pub(crate) struct Served {
    /// The index.
    pub(crate) index: Arc<PreconIndex>,
    /// Whether it was judged under other cards, precons or rules than the current ones, which are being judged.
    pub(crate) judging: bool,
}

impl Served {
    fn current(index: Arc<PreconIndex>) -> Served {
        Served {
            index,
            judging: false,
        }
    }
}

impl PreconCache {
    /// The current index, or the one held from before while the current one is built.
    pub(crate) async fn index(&self, source: &CardSource) -> Result<Arc<PreconIndex>, PreconError> {
        Ok(self.served(source).await?.index)
    }

    /// [`PreconCache::index`], saying whether it is the current one.
    pub(crate) async fn served(&self, source: &CardSource) -> Result<Served, PreconError> {
        let stamp = Stamp::now(source).await?;
        if let Some(index) = self.current(&stamp) {
            return Ok(Served::current(index));
        }
        let kept = self.load_kept(source).await;
        let (held, building) = {
            let mut held = slots(&self.slots);
            if held.built.is_none()
                && let Some((fingerprint, index)) = kept
                && stamp.fingerprint(source).as_ref() == Some(fingerprint)
            {
                held.built = Some((stamp.clone(), index.clone()));
            }
            if let Some((built, index)) = held.built.as_ref()
                && *built == stamp
            {
                return Ok(Served::current(index.clone()));
            }
            let building = match &held.building {
                Some((building, build)) if *building == stamp => build.clone(),
                _ => {
                    let build = self.start(source.clone(), stamp.clone());
                    held.building = Some((stamp, build.clone()));
                    build
                }
            };
            let built = held.built.as_ref().map(|(_, index)| index.clone());
            let held = built.or_else(|| kept.as_ref().map(|(_, index)| index.clone()));
            (held, building)
        };
        let failed = |message: String| PreconError::Catalog(CatalogError::Rules(message));
        match held {
            None => finished(building)
                .await
                .map(Served::current)
                .map_err(failed),
            Some(held) => match tokio::time::timeout(FRESH_WAIT, finished(building)).await {
                Ok(built) => built.map(Served::current).map_err(failed),
                Err(_) => Ok(Served {
                    index: held,
                    judging: true,
                }),
            },
        }
    }

    /// The index built for `stamp`, if it is held.
    fn current(&self, stamp: &Stamp) -> Option<Arc<PreconIndex>> {
        let held = slots(&self.slots);
        held.built
            .as_ref()
            .filter(|(built, _)| built == stamp)
            .map(|(_, index)| index.clone())
    }

    /// The index kept in the database, read once per process; none when there is none, it does not read or
    /// `source` keeps no judgements.
    pub(crate) async fn load_kept(&self, source: &CardSource) -> &Kept {
        self.kept.get_or_init(|| read_kept(source)).await
    }

    /// Starts a build of the index for `stamp`. When it is done it is kept, in memory and in the database, unless a
    /// build for another stamp was started meanwhile; a failed build is forgotten, so the next request tries again.
    fn start(&self, source: CardSource, stamp: Stamp) -> Building {
        let held = self.slots.clone();
        let (done, building) = watch::channel(None);
        tokio::spawn(async move {
            let started = Instant::now();
            let built = build(&source, stamp.clone()).await;
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis(),
                built = built.is_ok(),
                "precon index built"
            );
            let current = {
                let mut held = slots(&held);
                let current = held
                    .building
                    .as_ref()
                    .is_none_or(|(building, _)| *building == stamp);
                if current {
                    held.building = None;
                    if let Ok((built, index)) = &built {
                        held.built = Some((built.clone(), index.clone()));
                    }
                }
                current
            };
            done.send_replace(Some(built.clone().map(|(_, index)| index)));
            if current
                && let Ok((built, index)) = built
                && let Some(fingerprint) = built.fingerprint(&source)
            {
                keep(&source, fingerprint, index).await;
            }
        });
        building
    }
}

/// Builds the index for `stamp` under the current rules; the stamp it was built for, with the version of those rules.
async fn build(source: &CardSource, stamp: Stamp) -> Result<(Stamp, Arc<PreconIndex>), String> {
    let rules = source.rules().await.map_err(|error| error.to_string())?;
    let details = source
        .database()
        .precons()
        .all()
        .await
        .map_err(|error| error.to_string())?;
    let stamp = Stamp {
        rules: rules.version().clone(),
        ..stamp
    };
    #[cfg(test)]
    let gated = source.clone();
    let index = tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        gated.pass_precon_gate();
        Arc::new(PreconIndex::build(&rules, &details))
    })
    .await
    .map_err(|error| format!("the precon index build stopped: {error}"))?;
    Ok((stamp, index))
}

/// The row of `precon_judgements`, read back; a failure is logged and reads as none.
async fn read_kept(source: &CardSource) -> Kept {
    source.kept_under()?;
    let started = Instant::now();
    let row = sqlx::query!("SELECT fingerprint, items FROM precon_judgements")
        .fetch_optional(source.database().pool())
        .await
        .inspect_err(|error| tracing::warn!(%error, "the kept precon judgements could not be read"))
        .ok()??;
    let index = off_runtime(move || PreconIndex::from_kept(&row.items))
        .await
        .map_err(|error| error.to_string())
        .and_then(|read| read.map_err(|error| error.to_string()))
        .inspect_err(|error| tracing::warn!(%error, "the kept precon judgements do not read"))
        .ok()?;
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis(),
        precons = index.decks.len(),
        "kept precon judgements read"
    );
    Some((row.fingerprint, Arc::new(index)))
}

/// Writes `index` to `precon_judgements` under `fingerprint`, in place of the row before; a failure is logged.
async fn keep(source: &CardSource, fingerprint: String, index: Arc<PreconIndex>) {
    let items = off_runtime(move || index.to_kept())
        .await
        .map_err(|error| error.to_string())
        .and_then(|items| items.map_err(|error| error.to_string()));
    let written = match items {
        Ok(items) => sqlx::query!(
            "INSERT INTO precon_judgements (fingerprint, items) VALUES ($1, $2)
             ON CONFLICT (singleton) DO UPDATE
             SET fingerprint = EXCLUDED.fingerprint, items = EXCLUDED.items, judged_at = now()",
            fingerprint,
            items
        )
        .execute(source.database().pool())
        .await
        .map(drop)
        .map_err(|error| error.to_string()),
        Err(error) => Err(error),
    };
    if let Err(error) = written {
        tracing::warn!(%error, "the precon judgements could not be kept");
    }
}

/// What a build of the index came to, once it is done.
async fn finished(mut building: Building) -> Result<Arc<PreconIndex>, String> {
    let done = building
        .wait_for(Option::is_some)
        .await
        .map_err(|_| "the precon index build stopped".to_owned())?;
    done.clone()
        .unwrap_or_else(|| Err("the precon index build stopped".to_owned()))
}

#[cfg(test)]
mod tests {
    //! Goal: a precon's code is its set and the slug of its name in lower-case ASCII words, cut to a bounded length and made unique by the deck's id, and entries that match no card make the deck not legal and played by hand, counted as unplayable in the zone they sit in.
    use mtg_cards::deck::{DeckState, Summary};
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("Adaptive Enchantment", "adaptive-enchantment")]
    #[case("Atraxa, Praetors' Voice", "atraxa-praetors-voice")]
    #[case("They’re  Just   Like Us!", "theyre-just-like-us")]
    #[case("  --Élan Vital--  ", "lan-vital")]
    #[case("日本語", "")]
    #[case("", "")]
    fn a_name_becomes_lower_case_ascii_words(#[case] name: &str, #[case] expected: &str) {
        assert_eq!(slug(name), expected);
    }

    #[test]
    fn a_code_is_the_set_and_the_name_and_a_taken_one_ends_in_the_id() {
        let decks = [
            (PreconDeckId(1), "C18", "Adaptive Enchantment"),
            (PreconDeckId(2), "C18", "Adaptive: Enchantment"),
            (PreconDeckId(3), "C18", "Adaptive-Enchantment"),
            (PreconDeckId(4), "", "日本語"),
            (PreconDeckId(5), "SLD", "20 Ways to Win"),
        ];
        assert_eq!(
            codes(&decks),
            [
                "c18-adaptive-enchantment",
                "c18-adaptive-enchantment-2",
                "c18-adaptive-enchantment-3",
                "precon",
                "sld-20-ways-to-win"
            ]
        );
    }

    #[test]
    fn a_long_name_is_cut_without_a_trailing_hyphen_and_codes_stay_unique() {
        let name = "word ".repeat(40);
        let decks = [
            (PreconDeckId(1), "X", name.as_str()),
            (PreconDeckId(2), "X", name.as_str()),
        ];
        let made = codes(&decks);
        assert!(made.iter().all(|code| code.len() <= MAX_CODE_CHARS + 4));
        assert!(!made[0].ends_with('-'));
        assert_ne!(made[0], made[1]);
        assert!(
            made.iter()
                .flat_map(|code| code.chars())
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        );
    }

    #[test]
    fn an_id_clash_of_a_suffixed_code_goes_on_to_the_next_free_one() {
        let decks = [
            (PreconDeckId(1), "A", "B"),
            (PreconDeckId(2), "A", "B-2"),
            (PreconDeckId(2), "A", "B"),
        ];
        let made = codes(&decks);
        assert_eq!(made, ["a-b", "a-b-2", "a-b-2-1"]);
    }

    fn report() -> DeckReport {
        DeckReport {
            deck_hash: String::new(),
            coverage_hash: String::new(),
            format: "commander".into(),
            legality: Vec::new(),
            cards: Vec::new(),
            summary: Summary {
                format: "commander".into(),
                state: DeckState::Verified,
                legal: true,
                counts: Counts {
                    total: 99,
                    verified: 99,
                    ..Counts::default()
                },
                zones: vec![ZoneCounts {
                    zone: DeckZone::Main,
                    counts: Counts {
                        total: 99,
                        verified: 99,
                        ..Counts::default()
                    },
                }],
                issues: 0,
                to_read: 0,
            },
        }
    }

    #[test]
    fn entries_without_a_card_warn_and_count_as_unplayable_in_their_zone() {
        let mut report = report();
        count_unresolved(
            &mut report,
            &[
                Unresolved {
                    zone: DeckZone::Commander,
                    name: "Ghost Commander".into(),
                    count: 1,
                },
                Unresolved {
                    zone: DeckZone::Main,
                    name: "Ghost".into(),
                    count: 2,
                },
            ],
        );
        let summary = &report.summary;
        assert_eq!(
            (summary.state, summary.legal),
            (DeckState::ManualPlay, false)
        );
        assert_eq!((summary.counts.total, summary.counts.unplayable), (102, 3));
        assert_eq!(summary.issues, 2);
        let zones: Vec<_> = summary
            .zones
            .iter()
            .map(|zone| (zone.zone, zone.counts.total, zone.counts.unplayable))
            .collect();
        assert_eq!(
            zones,
            [(DeckZone::Main, 101, 2), (DeckZone::Commander, 1, 1)]
        );
        assert_eq!(report.legality[0].code, IssueCode::UnknownCard);
        assert_eq!(report.legality[0].name.as_deref(), Some("Ghost Commander"));
    }

    #[test]
    fn a_report_without_unresolved_entries_is_left_alone() {
        let mut report = report();
        let before = report.clone();
        count_unresolved(&mut report, &[]);
        assert_eq!(report, before);
    }
}

#[cfg(test)]
mod index_tests;

#[cfg(test)]
mod kept_tests;
