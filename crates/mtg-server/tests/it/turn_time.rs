//! Goal: through the actor, on tokio's paused time, each seat's turn time is the wall time of its own turns from start to the next turn's start, adds up across turns and seats, is public, ticks for the running seat only, is told to a seat that comes back, and stands still while no seat is connected.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::time::Duration;

use common::{Client, game_id, passive, roomy};
use mtg_engine::Decision;
use mtg_engine::state::Stage;
use mtg_protocol::{ClientMessage, DecisionId, ServerMessage, TableView};
use mtg_server::{GameConfig, GameHandle, Pace, spawn_game_with};
use mtg_testkit::Scenario;
use rstest::rstest;

struct Table {
    handle: GameHandle,
    clients: Vec<Option<Client>>,
    /// The latest table each seat was sent, from a snapshot or a batch.
    tables: Vec<Option<TableView>>,
    /// The latest decision each seat was sent.
    asked: Vec<Option<(DecisionId, Decision)>>,
}

impl Table {
    async fn start(seats: usize) -> Table {
        let setup = Scenario::new(seats).setup().unwrap();
        let config = GameConfig {
            pace: Pace::MANUAL,
            ..roomy()
        };
        let handle = spawn_game_with(game_id(1), setup, common::FlakyStore::new(), config).unwrap();
        let mut clients = Vec::new();
        for seat in 0..seats {
            clients.push(Some(Client::join(&handle, seat).await));
        }
        for client in clients.iter().flatten() {
            client.say(&handle, ClientMessage::Ready).await;
        }
        let mut table = Table {
            handle,
            clients,
            tables: vec![None; seats],
            asked: vec![None; seats],
        };
        table.settle().await;
        table
    }

    /// Lets the actor run until nothing is left to do at this instant, then reads what every seat was sent.
    async fn settle(&mut self) {
        for _ in 0..20 {
            tokio::task::yield_now().await;
            self.handle.status().await.unwrap();
        }
        for (seat, client) in self.clients.iter_mut().enumerate() {
            let Some(client) = client else { continue };
            for message in client.drain() {
                match message {
                    ServerMessage::Snapshot { state, .. } => self.tables[seat] = Some(state.table),
                    ServerMessage::Events(batch) => self.tables[seat] = Some(*batch.table),
                    ServerMessage::Decision { id, decision, .. } => {
                        self.asked[seat] = Some((id, decision));
                    }
                    _ => {}
                }
            }
        }
    }

    async fn wait(&mut self, millis: u64) {
        tokio::time::advance(Duration::from_millis(millis)).await;
        self.settle().await;
    }

    /// The turn number of the latest table seat 0 holds; 0 before the turns begin.
    fn turn(&self) -> u32 {
        self.tables[0]
            .as_ref()
            .filter(|table| table.stage == Stage::Turns)
            .map_or(0, |table| table.turn.number)
    }

    /// Every seat's (turn time, running) as `viewer` last heard it.
    fn figures(&self, viewer: usize) -> Vec<(u64, bool)> {
        self.tables[viewer]
            .as_ref()
            .expect("the seat holds a table")
            .players
            .iter()
            .map(|player| (player.turn_time_ms, player.turn_time_running))
            .collect()
    }

    /// Answers whatever is pending, passively.
    async fn answer(&mut self) {
        let seat = self
            .handle
            .status()
            .await
            .unwrap()
            .waiting_on
            .expect("a seat to wait for");
        let (id, decision) = self.asked[seat.index()].take().expect("the seat was asked");
        let client = self.clients[seat.index()]
            .as_ref()
            .expect("the seat is here");
        let message = ClientMessage::Answer {
            decision: id,
            answer: passive(&decision),
        };
        client.say(&self.handle, message).await;
        self.settle().await;
    }

    /// Plays until turn `last` has begun, spending `delay(turn)` ms at the start of every turn before it.
    async fn play(&mut self, last: u32, delay: impl Fn(u32) -> u64) {
        let mut seen = 0;
        for _ in 0..4000 {
            let turn = self.turn();
            if turn >= last {
                return;
            }
            if turn > seen {
                seen = turn;
                self.wait(delay(turn)).await;
            }
            self.answer().await;
        }
        panic!("the game never got there");
    }

    async fn leave(&mut self, seat: usize) {
        self.clients[seat] = None;
        self.handle.leave(common::seat(seat)).await;
        self.settle().await;
    }

    async fn come_back(&mut self, seat: usize) {
        self.clients[seat] = Some(Client::join(&self.handle, seat).await);
        self.settle().await;
    }
}

/// Seconds spent in turn `turn`, different for every turn.
fn delay(turn: u32) -> u64 {
    u64::from(turn) * 1_000 + 250
}

/// What every seat should have used at the start of turn `last`: the delays of its finished turns.
fn expected(seats: usize, last: u32) -> Vec<(u64, bool)> {
    let mut used = vec![(0, false); seats];
    for turn in 1..last {
        used[(turn as usize - 1) % seats].0 += delay(turn);
    }
    used[(last as usize - 1) % seats].1 = true;
    used
}

#[rstest]
#[case::two_seats(2, 5)]
#[case::three_seats(3, 7)]
#[case::four_seats(4, 6)]
#[tokio::test(start_paused = true)]
async fn turn_times_add_up_per_seat_and_are_public(#[case] seats: usize, #[case] last: u32) {
    let mut table = Table::start(seats).await;
    table.play(last, delay).await;
    for viewer in 0..seats {
        assert_eq!(
            table.figures(viewer),
            expected(seats, last),
            "seat {viewer}'s view"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_seat_that_comes_back_is_told_the_running_figure() {
    let mut table = Table::start(2).await;
    table.play(3, delay).await;
    // Turn 3, seat 0's, has just begun. Seat 1 leaves for 4 s while seat 0 stays.
    table.leave(1).await;
    table.wait(4_000).await;
    table.come_back(1).await;
    let mut want = expected(2, 3);
    want[0].0 += 4_000;
    assert_eq!(table.figures(1), want);
}

#[tokio::test(start_paused = true)]
async fn time_with_nobody_connected_does_not_count() {
    let mut table = Table::start(2).await;
    table.play(2, delay).await;
    table.leave(0).await;
    table.leave(1).await;
    table.wait(60_000).await;
    table.come_back(1).await;
    table.wait(500).await;
    table.come_back(0).await;
    let mut want = expected(2, 2);
    want[1].0 += 500;
    assert_eq!(
        table.figures(0),
        want,
        "the minute nobody was here is not counted"
    );
}
