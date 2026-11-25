//! `GET /ws/delves/{id}`: a signed-in user's socket that follows a run. The first frame is the run brought up to
//! date, as `GET /api/delves/{id}` answers it; every later frame is the run again after its log grew. The client
//! sends nothing but the closing handshake and pongs.

use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::rejection::PathRejection;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use mtg_protocol::rest::DelveView;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{mpsc, watch};
use tokio::time::{MissedTickBehavior, interval_at, timeout};
use uuid::Uuid;

use super::{fresh_view, id_of, service};
use crate::api::ApiError;
use crate::config::SocketLimits;
use crate::origin::origin_allowed;
use crate::server::AppState;
use crate::user_id::UserId;

/// Why a following socket ends, as its close frame says it.
#[derive(Debug, Clone, Copy)]
struct Goodbye(u16, &'static str);

const SHUTTING_DOWN: Goodbye = Goodbye(1001, "server shutting down");
const IDLE: Goodbye = Goodbye(1008, "no answer to ping");
const UNREADABLE: Goodbye = Goodbye(1011, "the run could not be read; reconnect");

/// The upgrade handler. A malformed address is 404 whoever asks; then no session is 401; then an unknown run is 404.
/// The first view is read before the upgrade, so a refusal is an HTTP answer.
pub(crate) async fn follow(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    if !origin_allowed(&headers, &app.config().allowed_origins) {
        return ApiError::forbidden_origin().into_response();
    }
    let Ok(id) = id_of(path) else {
        return ApiError::not_found().into_response();
    };
    let delves = match service(&app) {
        Ok(delves) => delves,
        Err(error) => return error.into_response(),
    };
    let user = match app.auth().require_user(&headers).await {
        Ok(user) => user.id,
        Err(error) => return error.into_response(),
    };
    if app.stopping() {
        return ApiError::unavailable().into_response();
    }
    // Subscribed before the first read, so a change while it is read is followed by a frame.
    let changes = delves.changed.subscribe();
    let first = match fresh_view(&app, delves, id, user).await {
        Ok(view) => view,
        Err(error) => return error.into_response(),
    };
    let upgrade = match upgrade {
        Ok(upgrade) => upgrade,
        Err(rejection) => return rejection.into_response(),
    };
    let limits = app.config().socket;
    let follower = Follower {
        id,
        user,
        changes,
        limits,
        stop: app.stop(),
        _guard: app.session_guard(),
    };
    upgrade
        .max_frame_size(limits.max_frame)
        .max_message_size(limits.max_frame)
        .on_upgrade(move |socket| run(socket, app, follower, first))
}

/// A socket's run, user and the changes it hears of.
struct Follower {
    id: Uuid,
    user: UserId,
    changes: Receiver<Uuid>,
    limits: SocketLimits,
    stop: watch::Receiver<bool>,
    /// Held while the socket lives, so shutdown waits for it.
    _guard: mpsc::Sender<()>,
}

async fn run(mut socket: WebSocket, app: AppState, mut follower: Follower, first: DelveView) {
    let goodbye = follow_run(&mut socket, &app, &mut follower, first).await;
    if let Some(Goodbye(code, reason)) = goodbye {
        let frame = CloseFrame {
            code,
            reason: reason.into(),
        };
        let close = socket.send(Message::Close(Some(frame)));
        let _ = timeout(follower.limits.send_timeout, close).await;
    }
}

/// Sends the run on every change until the socket ends; answers the close to send, if any.
async fn follow_run(
    socket: &mut WebSocket,
    app: &AppState,
    follower: &mut Follower,
    first: DelveView,
) -> Option<Goodbye> {
    let limits = follower.limits;
    if !send(socket, &first, limits.send_timeout).await {
        return None;
    }
    let period = limits.ping_every.max(Duration::from_millis(1));
    let mut pings = interval_at(tokio::time::Instant::now() + period, period);
    pings.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last_pong = Instant::now();
    loop {
        tokio::select! {
            _ = follower.stop.wait_for(|stopping| *stopping) => return Some(SHUTTING_DOWN),
            change = follower.changes.recv() => {
                match change {
                    Ok(id) if id != follower.id => continue,
                    // Fell behind: this run may be among the changes missed.
                    Ok(_) | Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => return Some(SHUTTING_DOWN),
                }
                // One read covers every change already heard of.
                while follower.changes.try_recv().is_ok() {}
                let Some(delves) = app.delves() else {
                    return Some(SHUTTING_DOWN);
                };
                let Ok(view) = fresh_view(app, delves, follower.id, follower.user).await else {
                    return Some(UNREADABLE);
                };
                if !send(socket, &view, limits.send_timeout).await {
                    return None;
                }
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Pong(_))) => last_pong = Instant::now(),
                Some(Ok(Message::Close(_)) | Err(_)) | None => return None,
                // The websocket layer answers pings; anything else the client sends is ignored.
                Some(Ok(_)) => {}
            },
            _ = pings.tick() => {
                if last_pong.elapsed() >= limits.idle_timeout {
                    return Some(IDLE);
                }
                let ping = socket.send(Message::Ping(Bytes::new()));
                if !matches!(timeout(limits.send_timeout, ping).await, Ok(Ok(()))) {
                    return None;
                }
            }
        }
    }
}

/// Sends `view` as one text frame; false when the client is gone or stalled.
async fn send(socket: &mut WebSocket, view: &DelveView, limit: Duration) -> bool {
    let Ok(text) = serde_json::to_string(view) else {
        tracing::error!("a delve view could not be serialized");
        return false;
    };
    matches!(
        timeout(limit, socket.send(Message::text(text))).await,
        Ok(Ok(()))
    )
}
