//! The deck endpoints: the decks a signed-in account stores, the deck check and the marker derived
//! from it, import of pasted lists and export as text, Arena or MTGO text. Everything here needs an
//! account; a server without a database answers 503 `decks_unavailable`.
//!
//! Saving a deck runs the check and stores `{deck_hash, coverage_hash, checked_at, summary}` with it.
//! Whether a deck is verified is never stored: it is derived each time the deck is read, from the
//! stored check and the check run at that moment (`mtg_cards::deck::marker`).

use std::collections::HashMap;

use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, Request, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use mtg_cards::deck::{DeckReport, StoredCheck, marker};
use mtg_core::house::HouseRules;
use mtg_db::{CreateDeckError, DeckDraft, DeckId, DeckRepo, StoredDeck};
use mtg_import::decklist::ExportFormat;
use mtg_protocol::rest::{
    CheckRequest, DeckBody, DeckErrorCode, DeckList, DeckListItem, DeckView, ImportRequest,
    ImportResult, MAX_DECKS_PER_USER, TableRules,
};
use serde_json::Value;
use sqlx::types::uuid::Uuid;

use crate::api::{ApiError, read_json_limited};
use crate::card_api::failed;
use crate::cards::CardSource;
use crate::deck_cards::{Line, Loaded, check_body, check_format, check_lines};
use crate::deck_choice::judge_at_table;
use crate::deck_import;
use crate::game_id::GameId;
use crate::recipe::FormatKind;
use crate::server::AppState;

/// Largest body of a deck, a check or a save, in bytes.
const DECK_BODY_LIMIT: usize = 64 * 1024;
/// Largest body of an import: a list of 100 000 characters, each escaped in JSON.
const IMPORT_BODY_LIMIT: usize = 640 * 1024;

fn code(code: DeckErrorCode) -> &'static str {
    code.as_str()
}

/// What every deck request has: the card source, the signed-in account's decks and its id.
struct Session<'a> {
    cards: &'a CardSource,
    repo: DeckRepo,
    owner: Uuid,
}

/// The 503 of a server without a database, then the 401 of a request without a session.
async fn session<'a>(app: &'a AppState, headers: &HeaderMap) -> Result<Session<'a>, ApiError> {
    let cards = app.cards().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            code(DeckErrorCode::DecksUnavailable),
            "this server runs without a database, so it has no decks",
        )
    })?;
    let user = app.auth().require_user(headers).await?;
    Ok(Session {
        cards,
        repo: cards.database().decks(),
        owner: user.id.uuid(),
    })
}

fn not_found() -> ApiError {
    ApiError::not_found()
}

/// The id of a deck in the path; one that is no id and one that is not the account's look alike.
fn deck_id(path: Result<Path<String>, PathRejection>) -> Result<DeckId, ApiError> {
    let Path(id) = path.map_err(|_| not_found())?;
    Uuid::parse_str(&id).map(DeckId).map_err(|_| not_found())
}

fn internal(message: &str) -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        code(DeckErrorCode::Internal),
        message,
    )
}

/// When a check ran, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// The JSON a save stores for a report: its hashes, the time and its summary.
fn check_json(report: &DeckReport) -> Result<Value, ApiError> {
    serde_json::to_value(StoredCheck::of(report, now())).map_err(|error| {
        tracing::error!(%error, "a check did not encode");
        internal("the deck could not be saved")
    })
}

/// The check stored with a deck; one that does not read (an older shape) counts as none.
fn stored_check(deck: &StoredDeck) -> Option<StoredCheck> {
    let value = deck.check.clone()?;
    match serde_json::from_value(value) {
        Ok(check) => Some(check),
        Err(error) => {
            tracing::warn!(deck = %deck.id, %error, "a stored check does not read; the deck counts as unchecked");
            None
        }
    }
}

/// The lines of a stored deck in the form the endpoints work with.
pub(crate) fn lines_of(deck: &StoredDeck) -> Vec<Line> {
    deck.lines.iter().map(Line::from_stored).collect()
}

/// A stored deck as the owner sees it: its cards, the check stored with it, the check of now and the
/// marker derived from the two.
fn deck_view(loaded: &Loaded, deck: &StoredDeck) -> DeckView {
    let lines = lines_of(deck);
    let report = loaded.report(&deck.format, &lines);
    let check = stored_check(deck);
    let mark = marker(check.as_ref(), &report);
    let mut views = Vec::with_capacity(lines.len());
    for line in &lines {
        views.extend(loaded.view(line));
    }
    DeckView {
        id: deck.id.to_string(),
        name: deck.name.clone(),
        format: deck.format.clone(),
        notes: deck.notes.clone(),
        created_at: deck.created_at.clone(),
        updated_at: deck.updated_at.clone(),
        cards: views,
        state: mark.state,
        verified: mark.verified,
        changed: mark.changed,
        check,
        report,
    }
}

fn list_item(loaded: &Loaded, deck: &StoredDeck) -> DeckListItem {
    let report = loaded.report(&deck.format, &lines_of(deck));
    let check = stored_check(deck);
    let mark = marker(check.as_ref(), &report);
    DeckListItem {
        id: deck.id.to_string(),
        name: deck.name.clone(),
        format: deck.format.clone(),
        created_at: deck.created_at.clone(),
        updated_at: deck.updated_at.clone(),
        state: mark.state,
        verified: mark.verified,
        changed: mark.changed,
        checked_at: check.map(|check| check.checked_at),
        summary: report.summary,
    }
}

/// A deck body checked, its cards read from the database and the deck checked: ready to be stored.
struct Prepared {
    draft: DeckDraft,
    loaded: Loaded,
}

async fn prepare(cards: &CardSource, body: &DeckBody) -> Result<Prepared, ApiError> {
    let checked = check_body(body)?;
    let loaded = Loaded::load(cards, &checked.lines).await?;
    loaded.require(&checked.lines)?;
    let report = loaded.report(&checked.format, &checked.lines);
    Ok(Prepared {
        draft: DeckDraft {
            name: checked.name,
            format: checked.format,
            notes: checked.notes,
            lines: checked.lines.iter().map(Line::to_stored).collect(),
            check: Some(check_json(&report)?),
        },
        loaded,
    })
}

fn save_failed(error: CreateDeckError) -> ApiError {
    match error {
        CreateDeckError::LimitReached { max } => ApiError::new(
            StatusCode::CONFLICT,
            code(DeckErrorCode::DeckLimit),
            format!("an account stores at most {max} decks; delete one first"),
        ),
        CreateDeckError::Database(error) => failed(&error),
    }
}

/// `GET /api/decks`: the account's decks, the most recently saved first, each with its derived state.
pub(crate) async fn list(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DeckList>, ApiError> {
    let s = session(&app, &headers).await?;
    let decks = s.repo.list(s.owner).await.map_err(|error| failed(&error))?;
    let lines: Vec<Line> = decks.iter().flat_map(lines_of).collect();
    let loaded = Loaded::load(s.cards, &lines).await?;
    let items = decks.iter().map(|deck| list_item(&loaded, deck)).collect();
    Ok(Json(DeckList { decks: items }))
}

/// `POST /api/decks`: stores a deck and the check run on it; 201 with the deck.
pub(crate) async fn create(
    State(app): State<AppState>,
    request: Request,
) -> Result<(StatusCode, Json<DeckView>), ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let headers = request.headers().clone();
    let s = session(&app, &headers).await?;
    let body: DeckBody = read_json_limited(request, DECK_BODY_LIMIT).await?;
    let prepared = prepare(s.cards, &body).await?;
    let stored = s
        .repo
        .create(s.owner, &prepared.draft, MAX_DECKS_PER_USER)
        .await
        .map_err(save_failed)?;
    let view = deck_view(&prepared.loaded, &stored);
    Ok((StatusCode::CREATED, Json(view)))
}

/// `GET /api/decks/{id}`: the deck with the check run now and the marker derived from it and from
/// the check stored at the last save.
pub(crate) async fn get_one(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<Json<DeckView>, ApiError> {
    let s = session(&app, &headers).await?;
    let id = deck_id(path)?;
    let deck = s
        .repo
        .get(s.owner, id)
        .await
        .map_err(|error| failed(&error))?
        .ok_or_else(not_found)?;
    let loaded = Loaded::load(s.cards, &lines_of(&deck)).await?;
    Ok(Json(deck_view(&loaded, &deck)))
}

/// `PUT /api/decks/{id}`: replaces the deck (name, format, notes and every line) and the check
/// stored with it.
pub(crate) async fn replace(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<Json<DeckView>, ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let headers = request.headers().clone();
    let s = session(&app, &headers).await?;
    let id = deck_id(path)?;
    let body: DeckBody = read_json_limited(request, DECK_BODY_LIMIT).await?;
    let prepared = prepare(s.cards, &body).await?;
    let stored = s
        .repo
        .replace(s.owner, id, &prepared.draft)
        .await
        .map_err(|error| failed(&error))?
        .ok_or_else(not_found)?;
    Ok(Json(deck_view(&prepared.loaded, &stored)))
}

/// `DELETE /api/decks/{id}`: deletes the deck; 204.
pub(crate) async fn remove(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let s = session(&app, &headers).await?;
    let id = deck_id(path)?;
    let deleted = s
        .repo
        .delete(s.owner, id)
        .await
        .map_err(|error| failed(&error))?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found())
    }
}

/// `POST /api/decks/check`: checks a stored deck (`{deck}`) or an unsaved list (`{format, cards}`)
/// and answers the report; nothing is saved.
pub(crate) async fn check(
    State(app): State<AppState>,
    request: Request,
) -> Result<Json<DeckReport>, ApiError> {
    let headers = request.headers().clone();
    let s = session(&app, &headers).await?;
    let body: CheckRequest = read_json_limited(request, DECK_BODY_LIMIT).await?;
    let table = table_of(&app, body.game.as_deref(), body.table.as_ref())?;
    let (format, lines) = match body {
        CheckRequest {
            deck: Some(id),
            format: None,
            cards: None,
            ..
        } => {
            let id = Uuid::parse_str(&id).map(DeckId).map_err(|_| not_found())?;
            let deck = s
                .repo
                .get(s.owner, id)
                .await
                .map_err(|error| failed(&error))?
                .ok_or_else(not_found)?;
            let lines = lines_of(&deck);
            (deck.format, lines)
        }
        CheckRequest {
            deck: None,
            format: Some(format),
            cards: Some(cards),
            ..
        } => (check_format(&format)?, check_lines(&cards)?),
        _ => {
            return Err(ApiError::invalid(
                "send either the id of a deck, or a format with the cards of a list",
            ));
        }
    };
    let loaded = Loaded::load(s.cards, &lines).await?;
    loaded.require(&lines)?;
    let mut report = loaded.report(&format, &lines);
    if let Some((kind, house)) = table {
        judge_at_table(s.cards, &lines, &mut report, kind, &house).await?;
    }
    Ok(Json(report))
}

/// The format and house rules a check is for: those of the game it names, or the table it
/// describes; none for a check of the deck alone. Naming both, an unknown game, a format the server
/// does not play or a rule out of range is refused.
fn table_of(
    app: &AppState,
    game: Option<&str>,
    table: Option<&TableRules>,
) -> Result<Option<(FormatKind, HouseRules)>, ApiError> {
    match (game, table) {
        (None, None) => Ok(None),
        (Some(id), None) => {
            let id: GameId = id.parse().map_err(|_| ApiError::not_found())?;
            app.registry()
                .deck_rules(id)
                .map(Some)
                .ok_or_else(ApiError::not_found)
        }
        (None, Some(table)) => {
            let format = FormatKind::parse(&table.format)
                .ok_or_else(|| ApiError::invalid("table.format is \"normal\" or \"commander\""))?;
            let house = table.house_rules.clone().unwrap_or_default();
            house.validate()?;
            Ok(Some((format, house)))
        }
        (Some(_), Some(_)) => Err(ApiError::invalid(
            "name the game or describe the table, not both",
        )),
    }
}

/// `POST /api/decks/import`: reads a pasted list in any format `mtg-import` reads and answers the
/// cards it resolves to and the lines it could not resolve; nothing is saved.
pub(crate) async fn import(
    State(app): State<AppState>,
    request: Request,
) -> Result<Json<ImportResult>, ApiError> {
    let headers = request.headers().clone();
    let s = session(&app, &headers).await?;
    let body: ImportRequest = read_json_limited(request, IMPORT_BODY_LIMIT).await?;
    Ok(Json(deck_import::import(s.cards, &body).await?))
}

/// `GET /api/decks/{id}/export?format=text|arena|mtgo`: the deck as a text file (`text` by default).
pub(crate) async fn export(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<HashMap<String, String>>, QueryRejection>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let s = session(&app, &headers).await?;
    let id = deck_id(path)?;
    let Query(params) = query.map_err(|_| ApiError::invalid("the query string is not valid"))?;
    if let Some(unknown) = params.keys().find(|key| key.as_str() != "format") {
        return Err(ApiError::invalid(format!("unknown parameter {unknown:?}")));
    }
    let format = match params.get("format").map(String::as_str) {
        None => ExportFormat::Text,
        Some(keyword) => ExportFormat::from_keyword(keyword)
            .ok_or_else(|| ApiError::invalid("format is text, arena or mtgo"))?,
    };
    let deck = s
        .repo
        .get(s.owner, id)
        .await
        .map_err(|error| failed(&error))?
        .ok_or_else(not_found)?;
    let loaded = Loaded::load(s.cards, &lines_of(&deck)).await?;
    let text = deck_import::write(&deck, &loaded, format)?;
    let disposition = format!(
        "attachment; filename=\"{}.txt\"",
        deck_import::file_stem(&deck.name)
    );
    let disposition = HeaderValue::from_str(&disposition)
        .map_err(|_| internal("the deck could not be written"))?;
    Ok((
        [
            (
                CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            ),
            (CONTENT_DISPOSITION, disposition),
        ],
        text,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    //! Goal: a deck path that is not an id is a plain 404, a stored check that does not read counts as none, and a check is stamped in the one form the wire documents.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("not-an-id")]
    #[case("")]
    #[case("4457ed35-7c10-48c8-9776-456485fdf07")]
    fn paths_that_are_no_ids_are_404(#[case] id: &str) {
        let error = deck_id(Ok(Path(id.to_owned()))).unwrap_err();
        assert_eq!(error.status(), StatusCode::NOT_FOUND);
        assert_eq!(error.code(), "not_found");
    }

    #[test]
    fn an_id_in_any_spelling_is_the_deck() {
        let lower = "4457ed35-7c10-48c8-9776-456485fdf070";
        let upper = lower.to_uppercase();
        assert_eq!(
            deck_id(Ok(Path(lower.to_owned()))).unwrap(),
            deck_id(Ok(Path(upper))).unwrap()
        );
    }

    #[test]
    fn the_time_of_a_check_has_the_documented_form() {
        let stamp = now();
        let bytes = stamp.as_bytes();
        assert_eq!(bytes.len(), 20, "{stamp}");
        assert!(stamp.ends_with('Z') && bytes[10] == b'T', "{stamp}");
        assert!(
            stamp
                .chars()
                .enumerate()
                .all(|(at, c)| matches!(at, 4 | 7 | 10 | 13 | 16 | 19) || c.is_ascii_digit()),
            "{stamp}"
        );
    }

    fn deck_with(check: Option<Value>) -> StoredDeck {
        StoredDeck {
            id: DeckId(Uuid::from_u128(1)),
            name: "d".into(),
            format: "freeform".into(),
            notes: String::new(),
            lines: Vec::new(),
            check,
            created_at: "2026-10-19T12:00:00Z".into(),
            updated_at: "2026-10-19T12:00:00Z".into(),
        }
    }

    #[test]
    fn a_stored_check_that_does_not_read_counts_as_none() {
        assert!(stored_check(&deck_with(None)).is_none());
        assert!(stored_check(&deck_with(Some(serde_json::json!({"deck_hash": 3})))).is_none());
        assert!(stored_check(&deck_with(Some(serde_json::json!({})))).is_none());
    }
}
