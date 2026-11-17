//! Load test: games of one passive person and three Very Hard bots play while a client asks the HTTP API, and the
//! request latencies are printed (`docs/performance.md`, Bot load).
//!
//! `cargo run --release -p mtg-server --example bot_load` with `GAMES` (default 3), `SECS` (default 60) and
//! `WORKERS` (runtime worker threads, default 4, as a small server has).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::bots::{BotLevel, BotSeat, safe_answer};
use mtg_server::registry::{GameOptions, SeatRequest};
use mtg_server::{GameHandle, Server, ServerConfig, Visibility, demo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Counts the bots' warnings (a brain past its deadline, a refused answer).
struct Warnings(Arc<AtomicUsize>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Warnings {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        let meta = event.metadata();
        if *meta.level() == tracing::Level::WARN && meta.target().starts_with("mtg_server::bots") {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
}

static WARNINGS: std::sync::OnceLock<Arc<AtomicUsize>> = std::sync::OnceLock::new();

fn main() {
    let warnings = Arc::clone(WARNINGS.get_or_init(|| Arc::new(AtomicUsize::new(0))));
    tracing_subscriber::registry()
        .with(Warnings(warnings))
        .init();
    let workers = usize::try_from(env("WORKERS", 4)).unwrap();
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .unwrap()
        .block_on(run());
}

async fn run() {
    let games = env("GAMES", 3);
    let secs = env("SECS", 60);
    let mut config = ServerConfig::default();
    config.auth.limits.lobby_requests_per_ip = u32::MAX;
    config.auth.limits.precon_requests_per_ip = u32::MAX;
    let server = Server::new(config);
    let registry = server.registry();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(server.serve(listener, std::future::pending()));

    let decks = ["mono-green", "mono-red"];
    let mut handles = Vec::new();
    for game in 0..games {
        let requests: Vec<SeatRequest> = (0..4)
            .map(|seat| SeatRequest {
                name: format!("Player {seat}"),
                deck: demo::find(decks[seat % 2]).unwrap(),
            })
            .collect();
        let options = GameOptions {
            visibility: Visibility::Public,
            bots: (1..4)
                .map(|seat| BotSeat {
                    seat: PlayerId::from_index(seat),
                    level: BotLevel::VeryHard,
                })
                .collect(),
            ..GameOptions::default()
        };
        let created = registry
            .create_with(&requests, options, None)
            .await
            .unwrap();
        let handle = registry.handle(created.id).unwrap();
        handles.push(handle.clone());
        tokio::spawn(person(handle));
        println!("game {game}: {}", created.id);
    }

    let mut lobbies = Vec::new();
    let mut options = Vec::new();
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        lobbies.push(timed_get(addr, "/api/lobbies").await);
        options.push(timed_get(addr, "/api/auth/options").await);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    report("GET /api/lobbies", &mut lobbies);
    report("GET /api/auth/options", &mut options);
    let mut events = 0;
    for handle in &handles {
        events += handle.status().await.map_or(0, |status| status.events);
    }
    let warnings = WARNINGS.get().map_or(0, |w| w.load(Ordering::Relaxed));
    println!("events in all games: {events}; bot warnings: {warnings}");
}

/// Seat 0: joins, says it is ready and gives the least answer to every decision at once.
async fn person(handle: GameHandle) {
    let seat = PlayerId::from_index(0);
    let mut inbox = handle.join(seat).await.unwrap();
    handle.send(seat, ClientMessage::Ready).await.unwrap();
    while let Some(message) = inbox.recv().await {
        if let ServerMessage::Decision { id, decision, .. } = message
            && decision.player == seat
        {
            let answer = safe_answer(&decision);
            let message = ClientMessage::Answer {
                decision: id,
                answer,
            };
            if handle.send(seat, message).await.is_err() {
                return;
            }
        }
    }
}

/// The time a plain HTTP/1.1 GET of `path` takes to answer in full.
async fn timed_get(addr: SocketAddr, path: &str) -> Duration {
    let started = Instant::now();
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).await.unwrap();
    assert!(reply.starts_with(b"HTTP/1.1 200"), "{path} failed");
    started.elapsed()
}

fn report(name: &str, times: &mut [Duration]) {
    times.sort();
    let at = |q: f64| {
        let index = ((times.len() as f64 - 1.0) * q).round() as usize;
        times[index].as_secs_f64() * 1000.0
    };
    println!(
        "{name}: n={} p50={:.1} ms p95={:.1} ms max={:.1} ms",
        times.len(),
        at(0.5),
        at(0.95),
        at(1.0)
    );
}
