//! Goal: start the real server on an ephemeral port and stop it again, keeping a handle on its registry.

use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use mtg_server::config::SocketLimits;
use mtg_server::{
    AuthConfig, CardSource, Dwell, GameConfig, HashCost, Pace, Registry, Server, ServerConfig,
};
use sqlx::PgPool;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// A config for bot games: roomy queues and a rate limit no bot reaches. Passwords hash at the
/// cheapest cost Argon2 allows, so tests with accounts stay fast.
pub fn bot_config() -> ServerConfig {
    ServerConfig {
        game: GameConfig {
            seat_queue: 8192,
            command_queue: 64,
            pace: Pace::MANUAL,
        },
        socket: SocketLimits {
            burst: 1_000_000,
            per_second: 1_000_000,
            ..SocketLimits::default()
        },
        auth: AuthConfig {
            hash_cost: HashCost::cheapest(),
            ..AuthConfig::default()
        },
        ..ServerConfig::default()
    }
}

/// [`bot_config`] with the default windows (every seat starts in Full Control `Off`) and no clocks.
pub fn windowed_config() -> ServerConfig {
    let config = bot_config();
    ServerConfig {
        game: GameConfig {
            pace: Pace {
                auto_pass: true,
                response: None,
                blocks: None,
                think: None,
                active: None,
                extension: None,
                grace: std::time::Duration::ZERO,
                dwell: Dwell::OFF,
            },
            ..config.game
        },
        ..config
    }
}

/// A running server plus the means to stop it.
pub struct TestServer {
    pub addr: SocketAddr,
    pub registry: Registry,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<io::Result<()>>>,
}

impl TestServer {
    /// Binds port 0 on localhost and serves `config` there.
    pub async fn start(config: ServerConfig) -> TestServer {
        TestServer::serve(Server::new(config)).await
    }

    /// Like [`TestServer::start`] on a server that records its games in `pool` and first resumes
    /// the ones an earlier server left unfinished.
    pub async fn start_on(pool: PgPool, config: ServerConfig) -> TestServer {
        TestServer::serve(Server::open(config, pool).await.unwrap()).await
    }

    /// Like [`TestServer::start_on`] with the card source given, for tests that need other rules or
    /// another idea of what the engine runs.
    pub async fn start_with_cards(
        pool: PgPool,
        config: ServerConfig,
        cards: CardSource,
    ) -> TestServer {
        TestServer::serve(Server::open_with(config, pool, cards).await.unwrap()).await
    }

    async fn serve(server: Server) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let registry = server.registry();
        let (stop, stopped) = oneshot::channel::<()>();
        let task = tokio::spawn(server.serve(listener, async move {
            // A dropped sender stops the server too.
            let _ = stopped.await;
        }));
        TestServer {
            addr,
            registry,
            stop: Some(stop),
            task: Some(task),
        }
    }

    /// Starts the graceful shutdown.
    pub fn shutdown(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }

    /// Waits for `serve` to return; a server that does not stop in time fails the test.
    pub async fn finished(&mut self) -> io::Result<()> {
        let task = self.task.take().expect("finished called once");
        tokio::time::timeout(Duration::from_secs(20), task)
            .await
            .expect("the server did not stop in time")
            .expect("the server task panicked")
    }
}
