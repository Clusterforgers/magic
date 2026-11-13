//! The admin's game routes: `GET /api/admin/games` lists the lobbies and running games, and
//! `POST /api/admin/games/{id}/end` ends one ([`Registry::end_by_admin`](crate::registry::Registry::end_by_admin)).

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, StatusCode};
use mtg_protocol::rest::{AdminGameList, EndGameBody, MAX_END_REASON_CHARS};

use crate::api::{ApiError, read_json};
use crate::game_id::GameId;
use crate::registry::AdminEnd;
use crate::server::AppState;

/// `GET /api/admin/games`: every lobby and running game, newest first; admin only.
pub(crate) async fn list(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AdminGameList>, ApiError> {
    app.auth().require_admin(&headers).await?;
    Ok(Json(AdminGameList {
        games: app.registry().admin_games().await,
    }))
}

/// The reason as the players read it: trimmed, none when blank, refused when too long.
fn reason_of(body: EndGameBody) -> Result<Option<String>, ApiError> {
    let Some(reason) = body.reason else {
        return Ok(None);
    };
    let reason = reason.trim();
    if reason.chars().count() > MAX_END_REASON_CHARS {
        return Err(ApiError::invalid(format!(
            "the reason is at most {MAX_END_REASON_CHARS} characters"
        )));
    }
    Ok((!reason.is_empty()).then(|| reason.to_owned()))
}

/// `POST /api/admin/games/{id}/end`: ends a lobby or running game (204); a game that already ended answers 204 too.
/// Admin only, judged before the body is read.
pub(crate) async fn end(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<StatusCode, ApiError> {
    let admin = app.auth().require_admin(request.headers()).await?;
    let Path(game) = path.map_err(|_| ApiError::not_found())?;
    let game: GameId = game.parse().map_err(|_| ApiError::not_found())?;
    let reason = reason_of(read_json::<EndGameBody>(request).await?)?;
    let ended = app
        .registry()
        .end_by_admin(game, reason)
        .await
        .map_err(|error| {
            tracing::warn!(%game, %error, "could not record a game an admin ended");
            ApiError::unavailable()
        })?;
    match ended {
        AdminEnd::Ended => {
            tracing::info!(%game, admin = %admin.id, "an admin ended a game");
            Ok(StatusCode::NO_CONTENT)
        }
        AdminEnd::AlreadyOver => Ok(StatusCode::NO_CONTENT),
        AdminEnd::NotFound => Err(ApiError::not_found()),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the reason an admin gives is trimmed, blank means none, and only one over the limit (in characters) is refused.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(None, None)]
    #[case(Some(String::new()), None)]
    #[case(Some("   ".to_owned()), None)]
    #[case(Some("  late night ".to_owned()), Some("late night".to_owned()))]
    #[case(Some("é".repeat(MAX_END_REASON_CHARS)), Some("é".repeat(MAX_END_REASON_CHARS)))]
    fn reasons_are_trimmed_and_blank_is_none(
        #[case] sent: Option<String>,
        #[case] kept: Option<String>,
    ) {
        let body = EndGameBody { reason: sent };
        assert_eq!(reason_of(body).ok(), Some(kept));
    }

    #[test]
    fn an_overlong_reason_is_refused() {
        let body = EndGameBody {
            reason: Some("x".repeat(MAX_END_REASON_CHARS + 1)),
        };
        let refused = reason_of(body).map_err(|error| error.status());
        assert_eq!(refused, Err(StatusCode::BAD_REQUEST));
    }
}
