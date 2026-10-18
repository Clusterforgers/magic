//! The game sockets: `GET /ws/{game}/{token}` for a seat's link and `GET /ws/{game}` for a signed-in
//! user's bound seat. Both authenticate before the upgrade and hand the socket to a session.

use axum::extract::rejection::PathRejection;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};

use crate::api::ApiError;
use crate::game_id::GameId;
use crate::origin::origin_allowed;
use crate::records::Binding;
use crate::registry::SeatAccess;
use crate::server::AppState;
use crate::session::{self, Session};

/// The seat-link upgrade handler. The token picks the seat before any frame is read; an unknown
/// game, a malformed address and a wrong token all get the same 404, so none of them can be told
/// apart. A signed-in visitor also takes an unclaimed seat for their account on the way in.
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
    claim(&app, &headers, &access).await;
    start(&app, access, upgrade)
}

/// The signed-in upgrade handler: the session cookie picks the user's own seat in the game, so a
/// bound seat can be rejoined without its link. A malformed address is 404 whoever asks; then no
/// session is 401; then an unknown game and a game the user has no seat in are one 404, so a
/// stranger learns nothing about which games exist or who sits in them.
pub(crate) async fn upgrade_as_user(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    if !origin_allowed(&headers, &app.config().allowed_origins) {
        return ApiError::forbidden_origin().into_response();
    }
    let Some(id) = path.ok().and_then(|Path(game)| game.parse::<GameId>().ok()) else {
        return ApiError::not_found().into_response();
    };
    let user = match app.auth().require_user(&headers).await {
        Ok(user) => user,
        Err(error) => return error.into_response(),
    };
    let Some(access) = app.registry().authenticate_user(id, user.id) else {
        return ApiError::not_found().into_response();
    };
    if app.stopping() {
        return ApiError::unavailable().into_response();
    }
    match upgrade {
        Ok(upgrade) => start(&app, access, upgrade),
        Err(rejection) => rejection.into_response(),
    }
}

/// Binds the seat a token opened to the visitor's account when the visitor is signed in and the
/// seat is nobody's. Whatever happens, the token has already opened the seat, so a failure here is
/// logged and the visitor plays on as before.
async fn claim(app: &AppState, headers: &HeaderMap, access: &SeatAccess) {
    // Not signed in, or a session that could not be checked (which was logged): a guest.
    let Ok(Some(user)) = app.auth().session_user(headers).await else {
        return;
    };
    match app
        .registry()
        .bind_seat(access.game, access.seat, user.id)
        .await
    {
        Ok(Binding::Bound) => {
            tracing::info!(game = %access.game, seat = access.seat.get(), user = %user.id, "seat bound to an account");
        }
        Ok(other) => {
            tracing::debug!(game = %access.game, seat = access.seat.get(), ?other, "seat left as it was");
        }
        Err(error) => {
            tracing::warn!(game = %access.game, %error, "could not bind a seat to an account");
        }
    }
}

fn start(app: &AppState, access: SeatAccess, upgrade: WebSocketUpgrade) -> Response {
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
