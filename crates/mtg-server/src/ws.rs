//! `GET /ws/{game}/{token}`: authenticates before the upgrade, then hands the socket to a session.

use axum::extract::rejection::PathRejection;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};

use crate::api::ApiError;
use crate::game_id::GameId;
use crate::origin::origin_allowed;
use crate::server::AppState;
use crate::session::{self, Session};

/// The upgrade handler. The token picks the seat before any frame is read; an unknown game, a
/// malformed address and a wrong token all get the same 404, so none of them can be told apart.
pub(crate) async fn upgrade(
    State(app): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
    headers: HeaderMap,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    if !origin_allowed(&headers, &app.config().allowed_origins) {
        return ApiError::forbidden_origin().into_response();
    }
    let access = path.ok().and_then(|Path((game, token))| {
        let id = game.parse::<GameId>().ok()?;
        app.registry().authenticate(id, &token)
    });
    let Some(access) = access else {
        return ApiError::not_found().into_response();
    };
    if app.stopping() {
        return ApiError::unavailable().into_response();
    }
    let upgrade = match upgrade {
        Ok(upgrade) => upgrade,
        Err(rejection) => return rejection.into_response(),
    };
    let limits = app.config().socket;
    let session = Session {
        access,
        limits,
        stop: app.stop(),
        _guard: app.session_guard(),
    };
    upgrade
        .max_frame_size(limits.max_frame)
        .max_message_size(limits.max_frame)
        .on_upgrade(move |socket| session::run(socket, session))
}
