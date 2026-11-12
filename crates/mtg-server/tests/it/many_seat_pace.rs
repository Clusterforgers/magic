//! Goal: the pace of play and what each seat is told hold at tables of 13 to 64 seats: one seat's response clock runs its whole length while every other seat is passed for and then the table answers for it, a seat earns an extension over three own turns and spends it at a deadline with every seat told, and a seat's sockets carry the names of its own hand and never another seat's.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use common::{Client, game_id, passive, roomy, seat};
use mtg_engine::{Decision, Event, GameSetup};
use mtg_protocol::{ClientMessage, DecisionId, FullControlMode, ServerMessage};
use mtg_server::{Dwell, EventStore, GameConfig, GameHandle, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::Scenario;
use mtg_testkit::cards::forest;
use rstest::rstest;

const RESPONSE: u64 = 10_000;
const EXTENSION: u64 = 20_000;

/// Auto-pass on, a response clock of 10 s that shows at once, an extension of 20 s and no dwell.
fn pace() -> Pace {
    Pace {
        auto_pass: true,
        response: Some(Duration::from_millis(RESPONSE)),
        blocks: Some(Duration::from_secs(45)),
        think: Some(Duration::from_mins(1)),
        active: None,
        extension: Some(Duration::from_millis(EXTENSION)),
        grace: Duration::ZERO,
        dwell: Dwell::OFF,
    }
}

struct Table {
    handle: GameHandle,
    store: MemoryStore,
    clients: Vec<Client>,
    /// Everything each seat was sent, in order.
    seen: Vec<Vec<ServerMessage>>,
}

impl Table {
    /// Every seat joins and is ready; the seats of `held` ask for Full Control first.
    async fn start(setup: GameSetup, held: &[usize]) -> Table {
        let seats = setup.players.len();
        let store = MemoryStore::new();
        let config = GameConfig {
            pace: pace(),
            ..roomy()
        };
        let handle = spawn_game_with(game_id(1), setup, store.clone(), config).unwrap();
        let mut clients = Vec::new();
        for n in 0..seats {
            clients.push(Client::join(&handle, n).await);
        }
        let mut table = Table {
            handle,
            store,
            clients,
            seen: vec![Vec::new(); seats],
        };
        for &n in held {
            table.full_control(n).await;
        }
        for client in &table.clients {
            client.say(&table.handle, ClientMessage::Ready).await;
        }
        table.settle().await;
        table
    }

    async fn full_control(&mut self, n: usize) {
        let mode = FullControlMode::Held;
        self.clients[n]
            .say(&self.handle, ClientMessage::FullControl { mode })
            .await;
        self.settle().await;
    }

    /// Lets the actor and its timers run until nothing is left at this instant, then keeps what every seat was sent.
    async fn settle(&mut self) {
        for _ in 0..20 {
            tokio::task::yield_now().await;
            self.handle.status().await.unwrap();
        }
        for (n, client) in self.clients.iter_mut().enumerate() {
            self.seen[n].extend(client.drain());
        }
    }

    async fn wait(&mut self, millis: u64) {
        tokio::time::advance(Duration::from_millis(millis)).await;
        self.settle().await;
    }

    async fn events(&self) -> u64 {
        self.handle.status().await.unwrap().events
    }

    async fn waiting(&self) -> usize {
        let status = self.handle.status().await.unwrap();
        status
            .waiting_on
            .expect("the game waits for a seat")
            .index()
    }

    /// The last decision `n` was sent, with its deadline.
    fn sent(&self, n: usize) -> (DecisionId, Decision, Option<u64>) {
        self.seen[n]
            .iter()
            .rev()
            .find_map(|message| match message {
                ServerMessage::Decision {
                    id,
                    decision,
                    deadline_ms,
                    ..
                } => Some((*id, decision.clone(), *deadline_ms)),
                _ => None,
            })
            .expect("the seat was sent a decision")
    }

    /// How many decisions `n` was sent.
    fn decisions(&self, n: usize) -> usize {
        self.seen[n]
            .iter()
            .filter(|m| matches!(m, ServerMessage::Decision { .. }))
            .count()
    }

    async fn answer(&mut self, n: usize) {
        let (id, decision, _) = self.sent(n);
        let message = ClientMessage::Answer {
            decision: id,
            answer: passive(&decision),
        };
        self.clients[n].say(&self.handle, message).await;
        self.settle().await;
    }

    /// The bank of every seat as `viewer` knows it from its batches and `Bank` messages.
    fn banks(&self, viewer: usize) -> Vec<u8> {
        let mut banks = vec![0; self.seen.len()];
        for message in &self.seen[viewer] {
            match message {
                ServerMessage::Snapshot { state, .. } => {
                    for player in &state.table.players {
                        banks[player.seat.index()] = player.bank;
                    }
                }
                ServerMessage::Events(batch) => {
                    for player in &batch.table.players {
                        banks[player.seat.index()] = player.bank;
                    }
                }
                ServerMessage::Bank { seat, bank } => banks[seat.index()] = *bank,
                _ => {}
            }
        }
        banks
    }

    fn bank_messages(&self, viewer: usize) -> Vec<(usize, u8)> {
        self.seen[viewer]
            .iter()
            .filter_map(|message| match message {
                ServerMessage::Bank { seat, bank } => Some((seat.index(), *bank)),
                _ => None,
            })
            .collect()
    }

    fn turn(&self) -> u32 {
        self.seen[0]
            .iter()
            .rev()
            .find_map(|message| match message {
                ServerMessage::Events(batch) => Some(batch.table.turn.number),
                _ => None,
            })
            .unwrap_or(1)
    }
}

fn passes_by(log: &[Event], n: usize) -> usize {
    log.iter()
        .filter(|e| matches!(e, Event::PriorityPassed { player } if *player == seat(n)))
        .count()
}

#[rstest]
#[tokio::test(start_paused = true)]
async fn one_seats_clock_runs_its_length_while_every_other_seat_is_passed_for(
    #[values(13, 32, 64)] seats: usize,
) {
    let last = seats - 1;
    let mut table = Table::start(Scenario::new(seats).setup().unwrap(), &[last]).await;
    assert_eq!(
        table.waiting().await,
        last,
        "the one seat in Full Control is asked"
    );
    let (_, decision, left) = table.sent(last);
    assert_eq!(decision.player, seat(last));
    assert_eq!(left, Some(RESPONSE));
    for n in 1..last {
        assert_eq!(table.decisions(n), 0, "seat {n} is passed for, never asked");
    }

    let events = table.events().await;
    let passed = passes_by(&table.store.load(game_id(1)).await.unwrap(), last);
    table.wait(RESPONSE - 1).await;
    assert_eq!(
        table.events().await,
        events,
        "the clock runs its whole length"
    );
    table.wait(1).await;
    let log = table.store.load(game_id(1)).await.unwrap();
    assert!(
        passes_by(&log, last) > passed,
        "the table passed for the seat at its deadline"
    );
}

#[rstest]
#[tokio::test(start_paused = true)]
async fn a_seat_earns_an_extension_and_spends_it_with_every_seat_told(
    #[values(13, 32, 64)] seats: usize,
) {
    let who = 1;
    let mut table = Table::start(Scenario::new(seats).setup().unwrap(), &[]).await;
    // Seat 1's third own turn ends when turn 2 * seats + 2 begins.
    let after = u32::try_from(2 * seats + 3).unwrap();
    for _ in 0..20_000 {
        if table.turn() >= after {
            break;
        }
        let n = table.waiting().await;
        table.answer(n).await;
    }
    assert!(table.turn() >= after, "the game never got there");
    for viewer in 0..seats {
        assert_eq!(table.banks(viewer)[who], 1, "seat {viewer} sees the bank");
    }

    table.full_control(who).await;
    let mut asked = None;
    for _ in 0..20_000 {
        let n = table.waiting().await;
        if n == who {
            asked = Some(table.sent(who));
            break;
        }
        table.answer(n).await;
    }
    let (id, _, left) = asked.expect("seat 1 is asked in Full Control");
    assert_eq!(left, Some(RESPONSE), "seat 1 is not the active player");
    let events = table.events().await;

    table.wait(RESPONSE).await;
    let (again, _, left) = table.sent(who);
    assert_eq!(again, id, "the same decision, sent again");
    assert_eq!(left, Some(EXTENSION));
    assert_eq!(table.events().await, events, "the table did not answer");
    for viewer in 0..seats {
        assert_eq!(
            table.bank_messages(viewer),
            [(who, 0)],
            "seat {viewer} is told"
        );
    }
    table.wait(EXTENSION).await;
    assert!(
        table.events().await > events,
        "the empty bank lets the table answer"
    );
}

/// A card only `owner` may know of while it is in that seat's hand.
fn secret(owner: usize, n: usize) -> Arc<mtg_cards::CardDefinition> {
    let mut card = (*forest()).clone();
    card.name = format!("Secret<p{owner}:c{n}>");
    Arc::new(card)
}

/// The seats whose secret cards `text` names.
fn named(text: &str) -> BTreeSet<usize> {
    text.match_indices("Secret<p")
        .filter_map(|(at, _)| {
            text[at + "Secret<p".len()..]
                .split(':')
                .next()?
                .parse()
                .ok()
        })
        .collect()
}

#[rstest]
#[tokio::test(start_paused = true)]
async fn a_seat_is_told_its_own_hand_and_never_another_seats(#[values(13, 32, 64)] seats: usize) {
    let mut scenario = Scenario::new(seats);
    for owner in 0..seats {
        scenario = scenario
            .hand(owner, (0..2).map(|n| secret(owner, n)))
            .library(owner, (2..6).map(|n| secret(owner, n)));
    }
    let mut table = Table::start(scenario.setup().unwrap(), &[]).await;
    // Every seat draws a card by hand, so each seat's batches carry the draws of all the others.
    for owner in 0..seats {
        let draw = ClientMessage::Manual {
            action: mtg_engine::manual::ManualAction::Draw { count: 1 },
        };
        table.clients[owner].say(&table.handle, draw).await;
    }
    table.settle().await;
    for viewer in 0..seats {
        let text: String = table.seen[viewer]
            .iter()
            .map(|m| serde_json::to_string(m).unwrap())
            .collect();
        assert_eq!(
            named(&text),
            BTreeSet::from([viewer]),
            "seat {viewer} hears only of its own hidden cards"
        );
    }
}
