//! Goal: start the real server on an ephemeral port and stop it again, keeping a handle on its registry.

use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use mtg_server::config::SocketLimits;
use mtg_server::{GameConfig, Pace, Registry, Server, ServerConfig};
use sqlx::PgPool;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// A config for bot games: roomy queues and a rate limit no bot reaches.
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
        ..ServerConfig::default()
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
