//! The deck-group endpoints: a signed-in account lists, creates, renames, collapses, reorders and deletes
//! the groups it sorts its decks into, and files a deck in one group or none. Deleting a group leaves its
//! decks ungrouped. Every route needs an account: 503 `decks_unavailable` on a server without a database,
//! then 401 `unauthenticated`; a group or deck of another account is a plain 404.

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, StatusCode};
use mtg_db::{CreateGroupError, DeckGroupRepo, DeckId, Filing, GroupChange, StoredGroup};
use mtg_protocol::rest::{
    DeckErrorCode, DeckGroupChange, DeckGroupChoice, DeckGroupOrder, DeckGroupView, DeckGroups,
    MAX_DECK_GROUP_NAME_CHARS, MAX_DECK_GROUPS_PER_USER, NewDeckGroup,
};
use sqlx::types::uuid::Uuid;

use crate::api::{ApiError, read_json_limited};
use crate::card_api::failed;
use crate::server::AppState;

/// Largest body of a group request, in bytes: an order of every group id is the largest.
const BODY_LIMIT: usize = 8 * 1024;

/// What every group request has: the account's groups and its id.
struct Session {
    repo: DeckGroupRepo,
    owner: Uuid,
}

/// The 503 of a server without a database, then the 401 of a request without a session.
async fn session(app: &AppState, headers: &HeaderMap) -> Result<Session, ApiError> {
    let cards = app.cards().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            DeckErrorCode::DecksUnavailable.as_str(),
            "this server runs without a database, so it has no decks",
        )
    })?;
    let user = app.auth().require_user(headers).await?;
    Ok(Session {
        repo: cards.database().deck_groups(),
        owner: user.id.uuid(),
    })
}

/// The id in the path; one that is no id and one of another account look alike.
fn id_of(path: Result<Path<String>, PathRejection>) -> Result<Uuid, ApiError> {
    let Path(id) = path.map_err(|_| ApiError::not_found())?;
    Uuid::parse_str(&id).map_err(|_| ApiError::not_found())
}

fn invalid(message: &str) -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        DeckErrorCode::InvalidRequest.as_str(),
        message,
    )
}

/// The name trimmed, 1 to [`MAX_DECK_GROUP_NAME_CHARS`] characters, or 400 `invalid_request`.
pub(crate) fn group_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_DECK_GROUP_NAME_CHARS {
        return Err(invalid(&format!(
            "a group's name has 1 to {MAX_DECK_GROUP_NAME_CHARS} characters"
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(invalid("a group's name has no control characters"));
    }
    Ok(name.to_owned())
}

fn view(group: StoredGroup) -> DeckGroupView {
    DeckGroupView {
        id: group.id.to_string(),
        name: group.name,
        collapsed: group.collapsed,
        decks: group.decks.into_iter().map(|id| id.to_string()).collect(),
    }
}

fn writable(app: &AppState) -> Result<(), ApiError> {
    if app.stopping() {
        Err(ApiError::unavailable())
    } else {
        Ok(())
    }
}

/// `GET /api/deck-groups`: the account's groups in its order, each with its decks.
pub(crate) async fn list(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DeckGroups>, ApiError> {
    let s = session(&app, &headers).await?;
    let groups = s.repo.list(s.owner).await.map_err(|error| failed(&error))?;
    Ok(Json(DeckGroups {
        groups: groups.into_iter().map(view).collect(),
    }))
}

/// `POST /api/deck-groups`: a new open group after the others; 201 with it, 409 `group_limit` at the cap.
pub(crate) async fn create(
    State(app): State<AppState>,
    request: Request,
) -> Result<(StatusCode, Json<DeckGroupView>), ApiError> {
    writable(&app)?;
    let headers = request.headers().clone();
    let s = session(&app, &headers).await?;
    let body: NewDeckGroup = read_json_limited(request, BODY_LIMIT).await?;
    let name = group_name(&body.name)?;
    let group = s
        .repo
        .create(s.owner, &name, MAX_DECK_GROUPS_PER_USER)
        .await
        .map_err(|error| match error {
            CreateGroupError::LimitReached { max } => ApiError::new(
                StatusCode::CONFLICT,
                DeckErrorCode::GroupLimit.as_str(),
                format!("an account keeps at most {max} deck groups"),
            ),
            CreateGroupError::Database(error) => failed(&error),
        })?;
    Ok((StatusCode::CREATED, Json(view(group))))
}

/// `PATCH /api/deck-groups/{id}`: renames, collapses or opens the group; 200 with it.
pub(crate) async fn change(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<Json<DeckGroupView>, ApiError> {
    writable(&app)?;
    let headers = request.headers().clone();
    let s = session(&app, &headers).await?;
    let id = id_of(path)?;
    let body: DeckGroupChange = read_json_limited(request, BODY_LIMIT).await?;
    let change = GroupChange {
        name: body.name.as_deref().map(group_name).transpose()?,
        collapsed: body.collapsed,
    };
    let group = s
        .repo
        .change(s.owner, id, &change)
        .await
        .map_err(|error| failed(&error))?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(view(group)))
}

/// `DELETE /api/deck-groups/{id}`: deletes the group, its decks stay and are ungrouped; 204.
pub(crate) async fn remove(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    writable(&app)?;
    let s = session(&app, &headers).await?;
    let id = id_of(path)?;
    if s.repo
        .delete(s.owner, id)
        .await
        .map_err(|error| failed(&error))?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found())
    }
}

/// `PUT /api/deck-groups/order`: puts the groups in the order of the body, which names each once; 204.
pub(crate) async fn reorder(
    State(app): State<AppState>,
    request: Request,
) -> Result<StatusCode, ApiError> {
    writable(&app)?;
    let headers = request.headers().clone();
    let s = session(&app, &headers).await?;
    let body: DeckGroupOrder = read_json_limited(request, BODY_LIMIT).await?;
    let ids = body
        .ids
        .iter()
        .map(|id| Uuid::parse_str(id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid("the order names group ids"))?;
    if s.repo
        .reorder(s.owner, &ids)
        .await
        .map_err(|error| failed(&error))?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(invalid("the order names each of the account's groups once"))
    }
}

/// `PUT /api/decks/{id}/group`: files the deck in the group, or in none for `null`; 204.
pub(crate) async fn file(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<StatusCode, ApiError> {
    writable(&app)?;
    let headers = request.headers().clone();
    let s = session(&app, &headers).await?;
    let deck = DeckId(id_of(path)?);
    let body: DeckGroupChoice = read_json_limited(request, BODY_LIMIT).await?;
    let group = body
        .group
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| ApiError::not_found())?;
    match s
        .repo
        .file(s.owner, deck, group)
        .await
        .map_err(|error| failed(&error))?
    {
        Filing::Done => Ok(StatusCode::NO_CONTENT),
        Filing::NoDeck | Filing::NoGroup => Err(ApiError::not_found()),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a group's name is trimmed, has 1 to 60 characters and no control characters.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("Fun", "Fun")]
    #[case("  Hardcore ", "Hardcore")]
    #[case(&"x".repeat(60), &"x".repeat(60))]
    fn names_are_trimmed(#[case] given: &str, #[case] kept: &str) {
        assert_eq!(group_name(given).unwrap(), kept);
    }

    #[rstest]
    #[case("")]
    #[case("   ")]
    #[case(&"x".repeat(61))]
    #[case("a\nb")]
    fn bad_names_are_400(#[case] given: &str) {
        let error = group_name(given).unwrap_err();
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error.code(), "invalid_request");
    }
}
