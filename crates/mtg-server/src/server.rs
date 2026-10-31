//! The HTTP server: routes, shared state and graceful shutdown.

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::header::{
    AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH, RETRY_AFTER,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderValue, Method};
use axum::middleware;
use axum::routing::{delete, get, post, put};
use axum::serve::ListenerExt;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::auth::{self, Auth};
use crate::card_api;
use crate::cards::CardSource;
use crate::config::ServerConfig;
use crate::images::{self, Images};
use crate::records::RecordError;
use crate::registry::{Registry, ResumeReport};
use crate::{api, decks, fix_marks, lobbies, my_games, precon_api, ws};

struct Inner {
    config: ServerConfig,
    registry: Registry,
    auth: Auth,
    /// The card database with the rules and what the engine runs; `None` without a database.
    cards: Option<CardSource>,
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

    /// The accounts: sign-in, sessions and the limits around them.
    pub(crate) fn auth(&self) -> &Auth {
        &self.inner.auth
    }

    /// The card database, for the card and deck endpoints; `None` on a server without a database.
    pub(crate) fn cards(&self) -> Option<&CardSource> {
        self.inner.cards.as_ref()
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
    resumed: ResumeReport,
}

impl Server {
    /// Builds a server that follows `config` and keeps its games in memory; nothing listens until
    /// [`Server::serve`]. Without a database there are no accounts: every account endpoint
    /// answers 503 and games are guest-only.
    pub fn new(config: ServerConfig) -> Server {
        let registry = Registry::new(&config);
        let auth = Auth::new(&config.auth, None);
        Server::with_registry(config, registry, auth, None, ResumeReport::default())
    }

    /// Builds a server that records its games and accounts in `pool` (already migrated by
    /// [`crate::migrate`]) and first brings back the games an earlier process left unfinished.
    ///
    /// A game that cannot be loaded is marked failed and skipped; only an unreachable database
    /// is an error.
    pub async fn open(config: ServerConfig, pool: PgPool) -> Result<Server, RecordError> {
        let cards = CardSource::new(pool.clone());
        Server::open_with(config, pool, cards).await
    }

    /// Like [`Server::open`] with the card source given, so the games, the card endpoints and the
    /// deck check share one set of rules and one idea of what the engine runs.
    pub async fn open_with(
        config: ServerConfig,
        pool: PgPool,
        cards: CardSource,
    ) -> Result<Server, RecordError> {
        let cards = if cards.images().is_some() {
            cards
        } else {
            cards.with_images(Images::new(&config.images))
        };
        let auth = Auth::new(&config.auth, Some(pool.clone()));
        auth.sync_admin()
            .await
            .map_err(|error| RecordError::Unavailable(error.to_string()))?;
        let registry = Registry::with_card_source(&config, pool, cards.clone());
        let resumed = registry.resume().await?;
        tracing::info!(
            resumed = resumed.resumed,
            lobbies = resumed.lobbies,
            finished = resumed.finished,
            expired = resumed.expired,
            failed = resumed.failed.len(),
            "stored games loaded"
        );
        Ok(Server::with_registry(
            config,
            registry,
            auth,
            Some(cards),
            resumed,
        ))
    }

    /// What loading the stored games did; empty for a server without a database.
    pub fn resumed(&self) -> &ResumeReport {
        &self.resumed
    }

    fn with_registry(
        config: ServerConfig,
        registry: Registry,
        auth: Auth,
        cards: Option<CardSource>,
        resumed: ResumeReport,
    ) -> Server {
        let (stop, stop_receiver) = watch::channel(false);
        let (sessions, drained) = mpsc::channel(1);
        let state = AppState {
            inner: Arc::new(Inner {
                config,
                registry,
                auth,
                cards,
                stop: stop_receiver,
                sessions,
            }),
        };
        Server {
            state,
            stop,
            drained,
            resumed,
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
            ..
        } = self;
        let grace = state.config().shutdown_grace;
        let mut stopping = state.stop();
        let janitor = state.registry().spawn_janitor(state.stop());
        let upkeep = state.auth().spawn_upkeep(state.stop());
        // Small request/answer messages should not wait for Nagle's algorithm.
        let listener = listener.tap_io(|tcp| {
            if let Err(error) = tcp.set_nodelay(true) {
                tracing::debug!(%error, "could not set TCP_NODELAY");
            }
        });
        // The peer address is what the login limits count when no proxy is trusted.
        let app = router(state).into_make_service_with_connect_info::<SocketAddr>();
        let serving = axum::serve(listener, app).with_graceful_shutdown(async move {
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
        if let Some(upkeep) = upkeep {
            upkeep.abort();
        }
        served
    }
}

/// The account routes: signing in and out, the settings of the signed-in account and the admin's.
fn account_routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/password", post(auth::change_password))
        .route("/api/auth/account", delete(auth::delete_account))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/options", get(auth::options))
        .route(
            "/api/admin/users",
            get(auth::list_accounts).post(auth::create_account),
        )
        .route("/api/admin/users/{id}", delete(auth::remove_account))
        .route("/api/admin/users/{id}/password", post(auth::reset_password))
}

/// All routes of the server.
pub(crate) fn router(state: AppState) -> Router {
    let origins = state
        .config()
        .allowed_origins
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok());
    // Listed origins are trusted front ends, so they may send the session cookie along.
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        // A seat token travels as a bearer token and the lobby list is polled with its `ETag`.
        .allow_headers([CONTENT_TYPE, AUTHORIZATION, IF_NONE_MATCH])
        // A page of a listed origin reads the list's tag and how long to wait after a 429.
        .expose_headers([ETAG, RETRY_AFTER])
        .allow_credentials(true)
        .max_age(Duration::from_secs(600));
    Router::new()
        .route("/healthz", get(api::healthz))
        .route("/api/demo-decks", get(api::decks))
        .route("/api/cards", get(card_api::search))
        .route("/api/cards/{oracle_id}", get(card_api::detail))
        .route(
            "/api/images/oracle/{oracle_id}/{face}/{size}",
            get(images::by_oracle),
        )
        .route(
            "/api/images/printing/{printing_id}/{face}/{size}",
            get(images::by_printing),
        )
        .route("/api/images/token/{face}/{size}", get(images::by_token))
        .route("/api/decks", get(decks::list).post(decks::create))
        .route("/api/decks/import", post(decks::import))
        .route("/api/decks/check", post(decks::check))
        .route(
            "/api/decks/{id}",
            get(decks::get_one)
                .put(decks::replace)
                .delete(decks::remove),
        )
        .route("/api/decks/{id}/export", get(decks::export))
        .route("/api/fix-marks", get(fix_marks::list))
        .route(
            "/api/fix-marks/{oracle_id}",
            put(fix_marks::put).delete(fix_marks::remove),
        )
        .route("/api/precons", get(precon_api::list))
        .route("/api/precons/{code}", get(precon_api::get_one))
        .route("/api/games", post(api::create_game))
        .route(
            "/api/games/{id}",
            get(api::game)
                .patch(lobbies::set_visibility)
                .delete(lobbies::close),
        )
        .route("/api/games/{id}/join", post(lobbies::join_game))
        .route("/api/games/{id}/leave", post(lobbies::leave))
        .route(
            "/api/games/{id}/seats/{seat}/release",
            post(lobbies::release),
        )
        .route(
            "/api/games/{id}/seats/{seat}/deck",
            put(lobbies::choose_deck),
        )
        .route("/api/join", post(lobbies::join_by_code))
        .route("/api/lobbies", get(lobbies::list))
        .merge(account_routes())
        .route("/api/me/games", get(my_games::list))
        .route("/api/me/games/{id}", delete(my_games::remove))
        .route("/ws/{game}/{token}", get(ws::upgrade))
        .route("/ws/{game}", get(ws::upgrade_as_user))
        .fallback(api::not_found)
        .method_not_allowed_fallback(api::method_not_allowed)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api::same_site_only,
        ))
        .layer(cors)
        // Responses carry tokens and live state: never cache them (the pictures say otherwise for
        // themselves), never sniff them.
        .layer(SetResponseHeaderLayer::if_not_present(
            CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .with_state(state)
}
