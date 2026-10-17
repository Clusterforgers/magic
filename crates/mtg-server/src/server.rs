//! The HTTP server: routes, shared state and graceful shutdown.

use std::future::Future;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
use axum::http::{HeaderValue, Method};
use axum::routing::{get, post};
use axum::serve::ListenerExt;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::config::ServerConfig;
use crate::registry::Registry;
use crate::{api, ws};

struct Inner {
    config: ServerConfig,
    registry: Registry,
    stop: watch::Receiver<bool>,
    sessions: mpsc::Sender<()>,
}

/// What every handler can reach. Cheap to clone.
#[derive(Clone)]
pub(crate) struct AppState {
    inner: Arc<Inner>,
}

impl AppState {
    pub(crate) fn config(&self) -> &ServerConfig {
        &self.inner.config
    }

    pub(crate) fn registry(&self) -> &Registry {
        &self.inner.registry
    }

    /// Whether shutdown has begun.
    pub(crate) fn stopping(&self) -> bool {
        *self.inner.stop.borrow()
    }

    /// A receiver that reports shutdown.
    pub(crate) fn stop(&self) -> watch::Receiver<bool> {
        self.inner.stop.clone()
    }

    /// A handle a socket holds while it lives, so shutdown can wait for it.
    pub(crate) fn session_guard(&self) -> mpsc::Sender<()> {
        self.inner.sessions.clone()
    }
}

/// A game server: the registry of games plus the HTTP and WebSocket front end.
pub struct Server {
    state: AppState,
    stop: watch::Sender<bool>,
    drained: mpsc::Receiver<()>,
}

impl Server {
    /// Builds a server that follows `config`; nothing listens until [`Server::serve`].
    pub fn new(config: ServerConfig) -> Server {
        let (stop, stop_receiver) = watch::channel(false);
        let (sessions, drained) = mpsc::channel(1);
        let registry = Registry::new(&config);
        let state = AppState {
            inner: Arc::new(Inner {
                config,
                registry,
                stop: stop_receiver,
                sessions,
            }),
        };
        Server {
            state,
            stop,
            drained,
        }
    }

    /// The registry of this server's games, for tests and admin tools.
    pub fn registry(&self) -> Registry {
        self.state.registry().clone()
    }

    /// The configuration in force.
    pub fn config(&self) -> &ServerConfig {
        self.state.config()
    }

    /// Serves `listener` until `shutdown` completes, then closes every socket with code 1001 and
    /// waits (up to the grace period) for them to finish.
    pub async fn serve(
        self,
        listener: TcpListener,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> io::Result<()> {
        let Server {
            state,
            stop,
            mut drained,
        } = self;
        let grace = state.config().shutdown_grace;
        let mut stopping = state.stop();
        let janitor = state.registry().spawn_janitor(state.stop());
        // Small request/answer messages should not wait for Nagle's algorithm.
        let listener = listener.tap_io(|tcp| {
            if let Err(error) = tcp.set_nodelay(true) {
                tracing::debug!(%error, "could not set TCP_NODELAY");
            }
        });
        let serving = axum::serve(listener, router(state)).with_graceful_shutdown(async move {
            shutdown.await;
            // Upgraded sockets outlive axum's own graceful shutdown, so each one is told to leave.
            let _ = stop.send(true);
        });
        let finish = async {
            let served = serving.await;
            // Every session and the router's state hold a sender; the channel closes when all are gone.
            let _ = drained.recv().await;
            served
        };
        // Whatever is still open when the grace period after the signal ends (a stalled request, a
        // peer that never answers the close frame) is abandoned, so a stop signal always stops.
        let deadline = async {
            let _ = stopping.wait_for(|stopping| *stopping).await;
            tokio::time::sleep(grace).await;
        };
        let served = tokio::select! {
            served = finish => served,
            () = deadline => {
                tracing::warn!("connections were still open when the shutdown grace period ended");
                Ok(())
            }
        };
        janitor.abort();
        served
    }
}

/// All routes of the server.
pub(crate) fn router(state: AppState) -> Router {
    let origins = state
        .config()
        .allowed_origins
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok());
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([CONTENT_TYPE])
        .max_age(Duration::from_secs(600));
    Router::new()
        .route("/healthz", get(api::healthz))
        .route("/api/decks", get(api::decks))
        .route("/api/games", post(api::create_game))
        .route("/api/games/{id}", get(api::game))
        .route("/ws/{game}/{token}", get(ws::upgrade))
        .fallback(api::not_found)
        .method_not_allowed_fallback(api::method_not_allowed)
        .layer(cors)
        // Responses carry tokens and live state: never cache them, never sniff them.
        .layer(SetResponseHeaderLayer::overriding(
            CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .with_state(state)
}
