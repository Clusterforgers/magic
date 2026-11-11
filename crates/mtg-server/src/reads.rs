//! The read endpoints: a signed-in account lists what it has left to read in a deck, a precon or every card, reads a card's
//! current draft beside its printed text (`mtg_cards::read::sheet`) and gives a verdict on it. A `wrong` needs a note
//! and also marks the card to fix. Every request counts against the account's budget
//! (`AttemptLimits::read_requests_per_user`); a server without a database answers 503 `cards_unavailable`.

use std::collections::{HashMap, HashSet};
use std::sync::PoisonError;

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode};
use mtg_cards::ReadState;
use mtg_cards::deck::{DeckReport, differs_of};
use mtg_cards::read::sheet;
use mtg_db::{Card, DeckId, DraftKey, IndexVersion, Verdict};
use mtg_import::ids::OracleId;
use mtg_protocol::rest::{
    CardDraft, DeckErrorCode, MyRead, ReadBody, ReadCard, ReadQueue, ReadQueueCard, ReadVerdict,
};
use sqlx::types::uuid::Uuid;

use crate::api::{ApiError, read_json_limited};
use crate::card_api::failed;
use crate::cards::{CardSource, Rules, StoredVersion, printed_card};
use crate::deck_cards::Loaded;
use crate::decks::lines_of;
use crate::fix_marks::{card_id, clean_note};
use crate::precon_api::judged_precon;
use crate::server::AppState;

/// Largest body of a verdict: a hash and a note of 280 characters, each escaped in JSON.
const READ_BODY_LIMIT: usize = 4 * 1024;

/// What every read request has: the card source and the signed-in account, within its budget.
pub(crate) struct Session<'a> {
    pub(crate) cards: &'a CardSource,
    pub(crate) user: Uuid,
}

/// The 503 of a server without a database, the 401 of a request without a session, then the 429 of an account over
/// its budget.
pub(crate) async fn session<'a>(
    app: &'a AppState,
    headers: &HeaderMap,
) -> Result<Session<'a>, ApiError> {
    let cards = app.cards().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            DeckErrorCode::CardsUnavailable.as_str(),
            "this server runs without a database, so it has no cards to read",
        )
    })?;
    let user = app.auth().require_user(headers).await?;
    app.auth().admit_read(user.id)?;
    Ok(Session {
        cards,
        user: user.id.uuid(),
    })
}

/// A card's current draft while it decides the card or would once read (no rules file or `Verified` row outranks it
/// and the engine runs it); `None` otherwise.
pub(crate) fn card_draft(rules: &Rules, card: &Card) -> Option<CardDraft> {
    let printed = printed_card(card);
    let state = rules.library().draft_state(&printed, rules.caps())?;
    let draft = rules.library().draft(&printed.oracle_id)?;
    Some(CardDraft {
        read_hash: draft.read_hash.clone(),
        state,
    })
}

fn verdict_of(verdict: ReadVerdict) -> Verdict {
    match verdict {
        ReadVerdict::Correct => Verdict::Correct,
        ReadVerdict::Wrong => Verdict::Wrong,
    }
}

fn wire_verdict(verdict: Verdict) -> ReadVerdict {
    match verdict {
        Verdict::Correct => ReadVerdict::Correct,
        Verdict::Wrong => ReadVerdict::Wrong,
    }
}

/// When a verdict was given, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
fn stamp(at: chrono::DateTime<chrono::Utc>) -> String {
    at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

impl Session<'_> {
    /// The stored card; 404 when the database lacks it.
    async fn stored(&self, id: &OracleId) -> Result<Card, ApiError> {
        let found = self
            .cards
            .database()
            .cards()
            .get_many(std::slice::from_ref(id))
            .await
            .map_err(|error| failed(&error))?;
        found.into_iter().next().ok_or_else(ApiError::not_found)
    }

    /// The card's current draft with its sheet, the counts on its hash and this account's verdict; 404 for a card
    /// without a draft that decides it.
    async fn card(&self, id: &OracleId) -> Result<ReadCard, ApiError> {
        let stored = self.stored(id).await?;
        let rules = self.cards.rules().await?;
        let found = card_draft(&rules, &stored).ok_or_else(ApiError::not_found)?;
        let draft = rules
            .library()
            .draft(id.as_str())
            .ok_or_else(ApiError::not_found)?;
        let printed = printed_card(&stored);
        let reads = self.cards.database().reads();
        let counts = reads
            .state_of(std::slice::from_ref(id))
            .await
            .map_err(|error| failed(&error))?
            .into_iter()
            .find(|row| row.read_hash == found.read_hash);
        let key = DraftKey {
            oracle_id: id.clone(),
            read_hash: found.read_hash.clone(),
        };
        let mine = reads
            .of_account(self.user, std::slice::from_ref(&key))
            .await
            .map_err(|error| failed(&error))?
            .into_iter()
            .next()
            .map(|read| MyRead {
                verdict: wire_verdict(read.verdict),
                note: read.note,
                created_at: stamp(read.created_at),
                current: read.current,
            });
        Ok(ReadCard {
            oracle_id: id.to_string(),
            name: stored.name.clone(),
            read_hash: found.read_hash,
            state: found.state,
            correct: counts.as_ref().map_or(0, |row| row.correct),
            wrong: counts.as_ref().map_or(0, |row| row.wrong),
            forge: draft.forge && !draft.differs,
            sheet: sheet(&draft.card, &printed),
            mine,
        })
    }

    /// What this account has left to read among the cards of `report`: pending drafts first, then approved ones it
    /// has not judged, each most played first.
    async fn deck_queue(&self, title: String, report: &DeckReport) -> Result<ReadQueue, ApiError> {
        let mut seen = HashSet::new();
        let mut waiting = Vec::new();
        for card in &report.cards {
            let waits = matches!(card.draft, Some(ReadState::Pending | ReadState::Approved));
            if waits && seen.insert(card.oracle_id.clone()) {
                let differs = u32::try_from(card.differs.len()).unwrap_or(u32::MAX);
                waiting.push(Waiting {
                    oracle_id: OracleId(card.oracle_id.clone()),
                    name: card.name.clone(),
                    differs,
                });
            }
        }
        self.queue(title, waiting, false).await
    }

    /// Every card whose current draft decides it and waits for a first read, most played first; with `mine`, only
    /// those in the account's decks. The library is walked off the runtime; nothing here judges a deck or a precon.
    async fn all_queue(&self, mine: bool) -> Result<ReadQueue, ApiError> {
        let mut waiting = self.all_waiting().await?;
        if mine {
            let held: HashSet<OracleId> = self
                .cards
                .database()
                .decks()
                .card_ids(self.user)
                .await
                .map_err(|error| failed(&error))?
                .into_iter()
                .collect();
            waiting.retain(|card| held.contains(&card.oracle_id));
        }
        self.queue(String::new(), waiting, true).await
    }

    /// The cards waiting for a first read, from the list held for the stamp of now when it is that list.
    async fn all_waiting(&self) -> Result<Vec<Waiting>, ApiError> {
        let rules = self.cards.rules().await?;
        let cards = self
            .cards
            .database()
            .cards()
            .index_version()
            .await
            .map_err(|error| failed(&error))?;
        let stamp = (rules.version().clone(), cards);
        let held = self.cards.pending_reads();
        if let Some(waiting) = held.get(&stamp) {
            return Ok(waiting);
        }
        let waiting = self.pending_now(rules).await?;
        held.keep(stamp, &waiting);
        Ok(waiting)
    }

    /// The cards whose current draft decides them and is pending, worked out again.
    async fn pending_now(&self, rules: Rules) -> Result<Vec<Waiting>, ApiError> {
        let candidates: Vec<OracleId> = rules
            .library()
            .drafts()
            .filter(|(_, draft)| draft.read == ReadState::Pending)
            .map(|(id, _)| OracleId(id.to_owned()))
            .collect();
        let stored = self
            .cards
            .database()
            .cards()
            .get_many(&candidates)
            .await
            .map_err(|error| failed(&error))?;
        tokio::task::spawn_blocking(move || pending_of(&rules, &stored))
            .await
            .map_err(|_| ApiError::unavailable())
    }

    /// The queue of `waiting` as the account stands: what it has not judged, by state and popularity, each card with
    /// its rank and whether one of the account's decks holds it. With `only_pending`, approved drafts are left out.
    async fn queue(
        &self,
        title: String,
        waiting: Vec<Waiting>,
        only_pending: bool,
    ) -> Result<ReadQueue, ApiError> {
        let db = self.cards.database();
        let ids: Vec<OracleId> = waiting.iter().map(|card| card.oracle_id.clone()).collect();
        let known: HashMap<OracleId, Waiting> = waiting
            .into_iter()
            .map(|card| (card.oracle_id.clone(), card))
            .collect();
        let rows = db
            .reads()
            .queue(self.user, &ids)
            .await
            .map_err(|error| failed(&error))?;
        let ranks = db.ranks().of(&ids).await.map_err(|error| failed(&error))?;
        let mine: HashSet<OracleId> = db
            .decks()
            .card_ids(self.user)
            .await
            .map_err(|error| failed(&error))?
            .into_iter()
            .collect();
        let cards = rows
            .into_iter()
            .filter(|row| !only_pending || row.state == mtg_db::ReadState::Pending)
            .filter_map(|row| {
                let card = known.get(&row.oracle_id)?;
                Some(ReadQueueCard {
                    oracle_id: row.oracle_id.to_string(),
                    name: card.name.clone(),
                    read_hash: row.read_hash,
                    state: wire_state(row.state),
                    differs: card.differs,
                    rank: ranks.get(&row.oracle_id).copied(),
                    in_decks: mine.contains(&row.oracle_id),
                })
            })
            .collect();
        Ok(ReadQueue { title, cards })
    }
}

/// What the pending cards were worked out under: the stored rules with the drafts and verdicts, and the cards.
type PendingStamp = (StoredVersion, IndexVersion);

/// The cards waiting for a first read, kept for the stamp they were worked out under, so a page load of the queue of
/// every card reads the library and the cards only after a draft, a verdict or an import changed them.
#[derive(Debug, Default)]
pub(crate) struct PendingReads {
    held: std::sync::Mutex<Option<(PendingStamp, Vec<Waiting>)>>,
}

impl PendingReads {
    /// The cards held for `stamp`, if these are they.
    fn get(&self, stamp: &PendingStamp) -> Option<Vec<Waiting>> {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        held.as_ref()
            .filter(|(at, _)| at == stamp)
            .map(|(_, waiting)| waiting.clone())
    }

    /// Keeps `waiting` as the cards of `stamp`.
    fn keep(&self, stamp: PendingStamp, waiting: &[Waiting]) {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        *held = Some((stamp, waiting.to_vec()));
    }
}

/// A card whose draft waits for a read, before the account's verdicts are known.
#[derive(Debug, Clone)]
struct Waiting {
    oracle_id: OracleId,
    name: String,
    differs: u32,
}

/// The stored cards whose current draft decides them under `rules` and is pending for their text today.
fn pending_of(rules: &Rules, stored: &[Card]) -> Vec<Waiting> {
    let library = rules.library();
    stored
        .iter()
        .filter_map(|card| {
            let printed = printed_card(card);
            let state = library.draft_state(&printed, rules.caps())?;
            (state == ReadState::Pending).then(|| Waiting {
                oracle_id: card.oracle_id.clone(),
                name: card.name.clone(),
                differs: u32::try_from(differs_of(library, &printed, rules.caps()).len())
                    .unwrap_or(u32::MAX),
            })
        })
        .collect()
}

/// A stored read state as the wire says it.
fn wire_state(state: mtg_db::ReadState) -> ReadState {
    match state {
        mtg_db::ReadState::Pending => ReadState::Pending,
        mtg_db::ReadState::Approved => ReadState::Approved,
        mtg_db::ReadState::Wrong => ReadState::Wrong,
    }
}

/// Which cards a list is for: at most one of `deck` (the account's own), `precon` (a code) and `mine=true` (the
/// account's decks); none is every card.
enum Target {
    Deck(DeckId),
    Precon(String),
    Mine,
    All,
}

fn target_of(params: &HashMap<String, String>) -> Result<Target, ApiError> {
    let given = |key: &str| params.get(key).filter(|value| !value.trim().is_empty());
    if params
        .keys()
        .any(|key| !matches!(key.as_str(), "deck" | "precon" | "mine"))
    {
        return Err(ApiError::invalid(
            "only `deck`, `precon` or `mine` may be given",
        ));
    }
    match (given("deck"), given("precon"), given("mine")) {
        (Some(id), None, None) => Uuid::parse_str(id.trim())
            .map(|uuid| Target::Deck(DeckId(uuid)))
            .map_err(|_| ApiError::not_found()),
        (None, Some(code), None) => Ok(Target::Precon(code.trim().to_ascii_lowercase())),
        (None, None, Some(flag)) if flag.trim() == "true" => Ok(Target::Mine),
        (None, None, None) if params.is_empty() => Ok(Target::All),
        _ => Err(ApiError::invalid(
            "name a `deck`, a `precon` or `mine=true`, or none",
        )),
    }
}

/// `GET /api/reads?deck=<id>` or `?precon=<code>`: what the account has left to read in that deck; `GET /api/reads`:
/// every card waiting for a first read; `?mine=true`: those of them in the account's decks. A deck of another account and an unknown code are 404.
pub(crate) async fn list(
    State(app): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<ReadQueue>, ApiError> {
    let s = session(&app, &headers).await?;
    let (title, report) = match target_of(&params)? {
        Target::All => return Ok(Json(s.all_queue(false).await?)),
        Target::Mine => return Ok(Json(s.all_queue(true).await?)),
        Target::Deck(id) => {
            let deck = s
                .cards
                .database()
                .decks()
                .get(s.user, id)
                .await
                .map_err(|error| failed(&error))?
                .ok_or_else(ApiError::not_found)?;
            let lines = lines_of(&deck);
            let loaded = Loaded::load(s.cards, &lines).await?;
            (
                deck.name.clone(),
                loaded.report_off(&deck.format, &lines).await?,
            )
        }
        Target::Precon(code) => {
            let (deck, judged) = judged_precon(s.cards, &code)
                .await?
                .ok_or_else(ApiError::not_found)?;
            (deck.summary.name, judged.report)
        }
    };
    Ok(Json(s.deck_queue(title, &report).await?))
}

/// `GET /api/reads/{oracle_id}`: the card's current draft with its read sheet and the account's verdict. A card
/// without a draft that decides it is 404.
pub(crate) async fn get_one(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<Json<ReadCard>, ApiError> {
    let s = session(&app, &headers).await?;
    let id = card_id(path)?;
    Ok(Json(s.card(&id).await?))
}

/// `PUT /api/reads/{oracle_id}`: the account's verdict on the draft the page showed; answers the card as it stands
/// after. A `wrong` needs a note and marks the card to fix with it. A hash other than the current draft's is 409
/// `stale_read`.
pub(crate) async fn put(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<Json<ReadCard>, ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let headers = request.headers().clone();
    let s = session(&app, &headers).await?;
    let id = card_id(path)?;
    let body: ReadBody = read_json_limited(request, READ_BODY_LIMIT).await?;
    let note = match body.verdict {
        ReadVerdict::Correct => None,
        ReadVerdict::Wrong => Some(
            clean_note(body.note)?
                .ok_or_else(|| ApiError::invalid("say what the draft does wrong"))?,
        ),
    };
    let stored = s.stored(&id).await?;
    let rules = s.cards.rules().await?;
    let current = card_draft(&rules, &stored).ok_or_else(ApiError::not_found)?;
    if current.read_hash != body.read_hash {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            DeckErrorCode::StaleRead.as_str(),
            "the card's draft changed since the page loaded it; read it again",
        ));
    }
    let key = DraftKey {
        oracle_id: id.clone(),
        read_hash: current.read_hash,
    };
    let db = s.cards.database();
    db.reads()
        .append(
            Some(s.user),
            &key,
            verdict_of(body.verdict),
            note.as_deref(),
        )
        .await
        .map_err(|error| failed(&error))?
        .ok_or_else(ApiError::not_found)?;
    if let Some(note) = note.as_deref() {
        db.fix_marks()
            .mark(s.user, &id, Some(note))
            .await
            .map_err(|error| failed(&error))?;
    }
    Ok(Json(s.card(&id).await?))
}

#[cfg(test)]
mod tests {
    //! Goal: a list names at most one deck, precon or `mine=true`, and a deck id that is no uuid is the 404 of an unknown deck.
    use rstest::rstest;

    use super::*;

    fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[rstest]
    #[case(&[("deck", " ")], 400)]
    #[case(&[("deck", "4457ed35-7c10-48c8-9776-456485fdf070"), ("precon", "c18")], 400)]
    #[case(&[("precon", "c18"), ("page", "2")], 400)]
    #[case(&[("mine", "false")], 400)]
    #[case(&[("mine", "")], 400)]
    #[case(&[("mine", "true"), ("precon", "c18")], 400)]
    #[case(&[("deck", "not-a-uuid")], 404)]
    fn a_list_names_one_target(#[case] pairs: &[(&str, &str)], #[case] status: u16) {
        let Err(error) = target_of(&params(pairs)) else {
            panic!("{pairs:?} names a target");
        };
        assert_eq!(error.status().as_u16(), status);
    }

    #[test]
    fn no_parameter_lists_every_card() {
        assert!(matches!(target_of(&params(&[])), Ok(Target::All)));
    }

    #[test]
    fn mine_true_lists_the_cards_of_the_accounts_decks() {
        assert!(matches!(
            target_of(&params(&[("mine", "true")])),
            Ok(Target::Mine)
        ));
    }

    #[test]
    fn a_precon_code_is_read_in_lower_case() {
        let Ok(Target::Precon(code)) = target_of(&params(&[("precon", " C18-Adaptive ")])) else {
            panic!("no precon");
        };
        assert_eq!(code, "c18-adaptive");
    }

    #[test]
    fn verdicts_map_both_ways() {
        for verdict in [ReadVerdict::Correct, ReadVerdict::Wrong] {
            assert_eq!(wire_verdict(verdict_of(verdict)), verdict);
        }
    }
}
