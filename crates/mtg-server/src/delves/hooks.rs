//! Hooks for browser tests, served only with `MTG_TEST_HOOKS` (`docs/server.md`): a test ends a boss fight early
//! without playing it. A server without the setting answers them 404 as if they did not exist.

use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use mtg_campaign::Status;
use mtg_core::ids::PlayerId;
use mtg_protocol::ClientMessage;

use super::{failure, id_of, load, not_found, refused, service};
use crate::api::ApiError;
use crate::game_id::GameId;
use crate::server::AppState;

/// `POST /api/test/delves/{id}/boss-concedes`: the boss of the run's fight concedes the running game, so the party
/// wins it by the normal rules and the run reads the win like any other. 204 once sent; 403 for anyone but the
/// host, 409 while no fight runs or its table has not started.
pub(crate) async fn boss_concedes(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    if !app.config().test_hooks {
        return Err(not_found());
    }
    let delves = service(&app)?;
    let user = app.auth().require_user(&headers).await?;
    let id = id_of(path)?;
    let loaded = load(delves, id).await?;
    if loaded.header.host != user.id.uuid() {
        return Err(failure(
            StatusCode::FORBIDDEN,
            "forbidden",
            "only the host can do that",
        ));
    }
    let Status::Fight { game: Some(game) } = &loaded.run.status else {
        return Err(refused("no fight is running"));
    };
    let handle = game
        .parse::<GameId>()
        .ok()
        .and_then(|game| app.registry().handle(game))
        .ok_or_else(|| refused("the fight has no table"))?;
    let started = handle.status().await.is_ok_and(|status| status.started);
    if !started {
        return Err(refused("the table has not started"));
    }
    // The boss's seat follows the heroes'.
    let boss = PlayerId::from_index(loaded.run.heroes.len());
    handle
        .send(boss, ClientMessage::Concede)
        .await
        .map_err(|_| refused("the table closed"))?;
    Ok(StatusCode::NO_CONTENT)
}
