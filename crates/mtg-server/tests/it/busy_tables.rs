//! Goal: a busy table never holds up the rest of the server: while a bot's brain thinks the runtime, the actor and the
//! lists answer at once (even on a single-threaded runtime), and while a table's write stalls a list takes the status
//! the table published last instead of waiting for it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{self, Client, FlakyStore, game_id, roomy};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use mtg_core::ids::PlayerId;
use mtg_engine::{Answer, Decision, Engine};
use mtg_protocol::ClientMessage;
use mtg_server::bots::{
    BotBrain, BotLevel, BotPace, BotSeat, Thinkers, safe_answer, spawn_bot_seat_with,
};
use mtg_server::registry::Removal;
use mtg_server::{MemoryStore, spawn_game_with};
use mtg_testkit::brain::SeatBrain;
use tokio::sync::watch;

/// How long a request may take while a table is busy.
const QUICK: Duration = Duration::from_millis(300);

/// A brain that holds its search for a long while, ignoring every stop, as a search on a crowded board might.
struct Slow {
    thinking: Arc<AtomicBool>,
}

impl SeatBrain for Slow {
    fn choose(&mut self, decision: &Decision, _table: Option<&Engine>) -> Answer {
        self.thinking.store(true, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(1500));
        self.thinking.store(false, Ordering::SeqCst);
        safe_answer(decision)
    }
}

#[tokio::test(flavor = "current_thread")]
async fn the_runtime_answers_while_a_bot_thinks() {
    let handle = spawn_game_with(
        game_id(1),
        common::table_setup(2, 3),
        MemoryStore::new(),
        roomy(),
    )
    .unwrap();
    let thinking = Arc::new(AtomicBool::new(false));
    let bot = BotSeat {
        seat: PlayerId(1),
        level: BotLevel::Normal,
    };
    let slow = Slow {
        thinking: Arc::clone(&thinking),
    };
    let first = BotBrain::new(3, bot.seat, bot.level)
        .with_brain(Box::new(slow))
        .with_deadline(Duration::from_secs(3));
    let (_alive, removed) = watch::channel(Removal::Dropped);
    let pace = BotPace {
        delay: Duration::from_millis(5),
        thinkers: Thinkers::new(1).for_game(),
    };
    let _bot = spawn_bot_seat_with(handle.clone(), bot, 3, first, pace, removed);
    let human = Client::join(&handle, 0).await;
    human.say(&handle, ClientMessage::Ready).await;
    common::timed(async {
        while !thinking.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;

    // The one runtime thread is free: a timer fires on time and the table answers at once.
    let asked = Instant::now();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let status = handle.status().await.unwrap();
    let took = asked.elapsed();
    assert!(
        thinking.load(Ordering::SeqCst),
        "the bot was still thinking"
    );
    assert!(status.started);
    assert!(took < QUICK, "the table took {took:?} to answer");
}

#[tokio::test]
async fn a_list_takes_the_last_published_status_of_a_table_whose_write_stalls() {
    let store = FlakyStore::new();
    let handle = spawn_game_with(
        game_id(2),
        common::table_setup(2, 5),
        store.clone(),
        roomy(),
    )
    .unwrap();
    let (mut clients, _) = common::start_table(&handle, 2).await;
    let before = handle.status().await.unwrap();
    assert!(before.started);

    // A slow COMMIT: the next answer's write holds the actor.
    store.stall(Duration::from_secs(3));
    let seat = before.waiting_on.unwrap().index();
    let (id, decision) = clients[seat].decision().await;
    let answer = common::passive(&decision);
    clients[seat]
        .say(
            &handle,
            ClientMessage::Answer {
                decision: id,
                answer,
            },
        )
        .await;

    // Asking the actor itself waits for the write ...
    let waited = tokio::time::timeout(Duration::from_millis(500), handle.status()).await;
    assert!(waited.is_err(), "the actor answered during its write");
    // ... while a list's question comes back at once with what the table published.
    let asked = Instant::now();
    let glanced = handle
        .status_within(Duration::from_millis(100))
        .await
        .unwrap();
    let took = asked.elapsed();
    assert!(took < QUICK, "the list waited {took:?}");
    assert!(glanced.started);
    assert_eq!(glanced.connected, before.connected);
    assert_eq!(glanced.state_hash, None, "a glance leaves the hash out");
}
