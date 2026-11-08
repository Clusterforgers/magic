//! The read endpoints: a signed-in account lists what it has left to read in a deck or a precon, reads a card's
//! current draft beside its printed text (`mtg_cards::read::sheet`) and gives a verdict on it. A `wrong` needs a note
//! and also marks the card to fix. Every request counts against the account's budget
//! (`AttemptLimits::read_requests_per_user`); a server without a database answers 503 `cards_unavailable`.

use std::collections::{HashMap, HashSet};

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode};
use mtg_cards::ReadState;
use mtg_cards::deck::DeckReport;
use mtg_cards::read::sheet;
use mtg_db::{Card, DeckId, DraftKey, Verdict};
use mtg_import::ids::OracleId;
use mtg_protocol::rest::{
    CardDraft, DeckErrorCode, MyRead, ReadBody, ReadCard, ReadQueue, ReadQueueCard, ReadVerdict,
};
use sqlx::types::uuid::Uuid;

use crate::api::{ApiError, read_json_limited};
use crate::card_api::failed;
use crate::cards::{CardSource, Rules, printed_card};
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
    /// has not judged, each in deck order.
    async fn queue(&self, title: String, report: &DeckReport) -> Result<ReadQueue, ApiError> {
        let mut seen = HashSet::new();
        let mut ids = Vec::new();
        let mut known: HashMap<String, (String, u32)> = HashMap::new();
        for card in &report.cards {
            let waits = matches!(card.draft, Some(ReadState::Pending | ReadState::Approved));
            if waits && seen.insert(card.oracle_id.clone()) {
                ids.push(OracleId(card.oracle_id.clone()));
                let differs = u32::try_from(card.differs.len()).unwrap_or(u32::MAX);
                known.insert(card.oracle_id.clone(), (card.name.clone(), differs));
            }
        }
        let rows = self
            .cards
            .database()
            .reads()
            .queue(self.user, &ids)
            .await
            .map_err(|error| failed(&error))?;
        let cards = rows
            .into_iter()
            .filter_map(|row| {
                let (name, differs) = known.get(row.oracle_id.as_str())?.clone();
                Some(ReadQueueCard {
                    oracle_id: row.oracle_id.to_string(),
                    name,
                    read_hash: row.read_hash,
                    state: wire_state(row.state),
                    differs,
                })
            })
            .collect();
        Ok(ReadQueue { title, cards })
    }
}

/// A stored read state as the wire says it.
fn wire_state(state: mtg_db::ReadState) -> ReadState {
    match state {
        mtg_db::ReadState::Pending => ReadState::Pending,
        mtg_db::ReadState::Approved => ReadState::Approved,
        mtg_db::ReadState::Wrong => ReadState::Wrong,
    }
}

/// Which deck a list is for: exactly one of `deck` (the account's own) and `precon` (a code).
enum Target {
    Deck(DeckId),
    Precon(String),
}

fn target_of(params: &HashMap<String, String>) -> Result<Target, ApiError> {
    let deck = params.get("deck").filter(|value| !value.trim().is_empty());
    let precon = params
        .get("precon")
        .filter(|value| !value.trim().is_empty());
    if params
        .keys()
        .any(|key| key.as_str() != "deck" && key.as_str() != "precon")
    {
        return Err(ApiError::invalid("only `deck` or `precon` may be given"));
    }
    match (deck, precon) {
        (Some(id), None) => Uuid::parse_str(id.trim())
            .map(|uuid| Target::Deck(DeckId(uuid)))
            .map_err(|_| ApiError::not_found()),
        (None, Some(code)) => Ok(Target::Precon(code.trim().to_ascii_lowercase())),
        _ => Err(ApiError::invalid("name either a `deck` or a `precon`")),
    }
}

/// `GET /api/reads?deck=<id>` or `?precon=<code>`: what the account has left to read in that deck. A deck of another
/// account and an unknown code are 404.
pub(crate) async fn list(
    State(app): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<ReadQueue>, ApiError> {
    let s = session(&app, &headers).await?;
    let (title, report) = match target_of(&params)? {
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
            (deck.name.clone(), loaded.report(&deck.format, &lines))
        }
        Target::Precon(code) => {
            let (deck, judged) = judged_precon(s.cards, &code)
                .await?
                .ok_or_else(ApiError::not_found)?;
            (deck.summary.name, judged.report)
        }
    };
    Ok(Json(s.queue(title, &report).await?))
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
    //! Goal: a list names exactly one deck or precon, and a deck id that is no uuid is the 404 of an unknown deck.
    use rstest::rstest;

    use super::*;

    fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[rstest]
    #[case(&[], 400)]
    #[case(&[("deck", " ")], 400)]
    #[case(&[("deck", "4457ed35-7c10-48c8-9776-456485fdf070"), ("precon", "c18")], 400)]
    #[case(&[("precon", "c18"), ("page", "2")], 400)]
    #[case(&[("deck", "not-a-uuid")], 404)]
    fn a_list_names_one_target(#[case] pairs: &[(&str, &str)], #[case] status: u16) {
        let Err(error) = target_of(&params(pairs)) else {
            panic!("{pairs:?} names a target");
        };
        assert_eq!(error.status().as_u16(), status);
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
