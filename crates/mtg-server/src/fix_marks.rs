//! The fix-mark endpoints: a signed-in account marks a card as important to fix (with an optional
//! short note), takes the mark back and lists its marks. The owner reads all accounts' marks ranked
//! through `mtg-cli fix-marks`; no endpoint shows another account's mark. A server without a
//! database answers 503 `cards_unavailable`.

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, StatusCode};
use mtg_db::{FixMark, FixMarkRepo};
use mtg_import::ids::OracleId;
use mtg_protocol::rest::{
    DeckErrorCode, FixMarkBody, FixMarkList, FixMarkView, MAX_FIX_NOTE_CHARS,
};
use sqlx::types::uuid::Uuid;

use crate::api::{ApiError, read_json_limited};
use crate::card_api::failed;
use crate::server::AppState;

/// Largest body of a mark: a note of 280 characters, each escaped in JSON.
const MARK_BODY_LIMIT: usize = 4 * 1024;

/// What every fix-mark request has: the signed-in account's marks and its id.
struct Session {
    repo: FixMarkRepo,
    user: Uuid,
}

/// The 503 of a server without a database, then the 401 of a request without a session.
async fn session(app: &AppState, headers: &HeaderMap) -> Result<Session, ApiError> {
    let cards = app.cards().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            DeckErrorCode::CardsUnavailable.as_str(),
            "this server runs without a database, so it has no cards to mark",
        )
    })?;
    let user = app.auth().require_user(headers).await?;
    Ok(Session {
        repo: cards.database().fix_marks(),
        user: user.id.uuid(),
    })
}

/// The id of a card in the path; one that is no id and one that names no card look alike.
fn card_id(path: Result<Path<String>, PathRejection>) -> Result<OracleId, ApiError> {
    let Path(id) = path.map_err(|_| ApiError::not_found())?;
    let uuid = Uuid::parse_str(&id).map_err(|_| ApiError::not_found())?;
    Ok(OracleId(uuid.to_string()))
}

/// The note of a body, trimmed: none when empty, refused when too long or holding control
/// characters other than line breaks.
fn note_of(body: FixMarkBody) -> Result<Option<String>, ApiError> {
    let Some(note) = body.note.map(|note| note.trim().to_owned()) else {
        return Ok(None);
    };
    if note.chars().count() > MAX_FIX_NOTE_CHARS {
        return Err(ApiError::invalid(format!(
            "the note is at most {MAX_FIX_NOTE_CHARS} characters"
        )));
    }
    if note.chars().any(|c| c.is_control() && c != '\n') {
        return Err(ApiError::invalid(
            "the note holds a control character other than a line break",
        ));
    }
    Ok((!note.is_empty()).then_some(note))
}

fn view_of(mark: FixMark) -> FixMarkView {
    FixMarkView {
        oracle_id: mark.oracle_id.to_string(),
        name: mark.name,
        note: mark.note,
        created_at: mark.created_at,
    }
}

/// `GET /api/fix-marks`: the account's marks, the newest first.
pub(crate) async fn list(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<FixMarkList>, ApiError> {
    let s = session(&app, &headers).await?;
    let marks = s.repo.list(s.user).await.map_err(|error| failed(&error))?;
    Ok(Json(FixMarkList {
        marks: marks.into_iter().map(view_of).collect(),
    }))
}

/// `PUT /api/fix-marks/{oracle_id}`: marks the card, or replaces the note of the mark the account
/// already made; answers the mark. An unknown card is 404.
pub(crate) async fn put(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<Json<FixMarkView>, ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let headers = request.headers().clone();
    let s = session(&app, &headers).await?;
    let card = card_id(path)?;
    let body: FixMarkBody = read_json_limited(request, MARK_BODY_LIMIT).await?;
    let note = note_of(body)?;
    let mark = s
        .repo
        .mark(s.user, &card, note.as_deref())
        .await
        .map_err(|error| failed(&error))?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(view_of(mark)))
}

/// `DELETE /api/fix-marks/{oracle_id}`: takes the account's mark back; 204, and 404 when it made none.
pub(crate) async fn remove(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let s = session(&app, &headers).await?;
    let card = card_id(path)?;
    let removed = s
        .repo
        .unmark(s.user, &card)
        .await
        .map_err(|error| failed(&error))?;
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found())
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a path that is no card id is a plain 404, and a note is trimmed, empty means none, and one over the limit or with a control character is refused.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("not-an-id")]
    #[case("")]
    #[case("4457ed35-7c10-48c8-9776-456485fdf07")]
    fn paths_that_are_no_ids_are_404(#[case] id: &str) {
        let error = card_id(Ok(Path(id.to_owned()))).unwrap_err();
        assert_eq!(error.status(), StatusCode::NOT_FOUND);
        assert_eq!(error.code(), "not_found");
    }

    #[test]
    fn an_id_in_any_spelling_is_the_card() {
        let lower = "4457ed35-7c10-48c8-9776-456485fdf070";
        assert_eq!(
            card_id(Ok(Path(lower.to_uppercase()))).unwrap(),
            card_id(Ok(Path(lower.to_owned()))).unwrap()
        );
    }

    fn body(note: Option<&str>) -> FixMarkBody {
        FixMarkBody {
            note: note.map(str::to_owned),
        }
    }

    #[rstest]
    #[case(None, None)]
    #[case(Some(""), None)]
    #[case(Some(" \n "), None)]
    #[case(Some(" it never draws\nthe card "), Some("it never draws\nthe card"))]
    fn notes_are_trimmed_and_empty_means_none(
        #[case] note: Option<&str>,
        #[case] stored: Option<&str>,
    ) {
        assert_eq!(note_of(body(note)).unwrap().as_deref(), stored);
    }

    #[test]
    fn the_limit_counts_characters_not_bytes() {
        assert!(note_of(body(Some(&"é".repeat(MAX_FIX_NOTE_CHARS)))).is_ok());
        let error = note_of(body(Some(&"é".repeat(MAX_FIX_NOTE_CHARS + 1)))).unwrap_err();
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error.code(), "invalid_request");
    }

    #[rstest]
    #[case("a\u{0}b")]
    #[case("a\u{7}")]
    #[case("tab\there")]
    fn control_characters_other_than_line_breaks_are_refused(#[case] note: &str) {
        assert!(note_of(body(Some(note))).is_err());
    }

    #[test]
    fn the_limit_is_the_database_s() {
        assert_eq!(MAX_FIX_NOTE_CHARS, mtg_db::MAX_NOTE_CHARS);
    }
}
