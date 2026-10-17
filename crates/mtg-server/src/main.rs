//! The game server binary: configuration from the environment, logs, graceful shutdown.

use std::process::ExitCode;

use mtg_server::{Server, ServerConfig};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

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
    tracing::info!(
        bind = %config.bind,
        max_games = config.max_games,
        allowed_origins = config.allowed_origins.len(),
        "listening"
    );
    Server::new(config)
        .serve(listener, shutdown_signal())
        .await
        .map_err(|error| format!("server failed: {error}"))?;
    tracing::info!("stopped");
    Ok(())
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
