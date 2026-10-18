//! The game server binary: configuration from the environment, logs, graceful shutdown.

use std::process::ExitCode;
use std::time::Duration;

use mtg_server::{Server, ServerConfig, migrate};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

/// Connections the game records and event logs share.
const DB_CONNECTIONS: u32 = 10;
/// How long a query waits for a free connection before the game it belongs to sees a failure.
const DB_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            tracing::error!("{message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let config = ServerConfig::from_env().map_err(|error| format!("configuration: {error}"))?;
    let listener = TcpListener::bind(config.bind)
        .await
        .map_err(|error| format!("cannot listen on {}: {error}", config.bind))?;
    let (bind, max_games, allowed_origins) =
        (config.bind, config.max_games, config.allowed_origins.len());
    let registration = format!("{:?}", config.auth.registration);
    let (cookie_secure, trusted_proxies) = (config.auth.cookie_secure, config.auth.trusted_proxies);
    let server = build(config).await?;
    tracing::info!(%bind, max_games, allowed_origins, %registration, cookie_secure, trusted_proxies, "listening");
    server
        .serve(listener, shutdown_signal())
        .await
        .map_err(|error| format!("server failed: {error}"))?;
    tracing::info!("stopped");
    Ok(())
}

/// Opens the database named by `DATABASE_URL` (migrating it and resuming its unfinished games),
/// or keeps every game in memory when the variable is unset.
async fn build(config: ServerConfig) -> Result<Server, String> {
    let url = std::env::var("DATABASE_URL")
        .ok()
        .filter(|url| !url.trim().is_empty());
    let Some(url) = url else {
        tracing::warn!("DATABASE_URL is not set: games live in memory and end with the process");
        return Ok(Server::new(config));
    };
    // The URL holds the password, so it is never logged.
    let pool = PgPoolOptions::new()
        .max_connections(DB_CONNECTIONS)
        .acquire_timeout(DB_ACQUIRE_TIMEOUT)
        .connect(&url)
        .await
        .map_err(|error| format!("cannot connect to the database: {error}"))?;
    migrate(&pool)
        .await
        .map_err(|error| format!("cannot migrate the database: {error}"))?;
    Server::open(config, pool)
        .await
        .map_err(|error| format!("cannot load the stored games: {error}"))
}

/// Completes on ctrl-c or SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutdown requested");
}
