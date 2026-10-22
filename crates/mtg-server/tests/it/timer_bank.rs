//! Goal: through the actor, on tokio's paused time, a seat earns one extension per three own turns finished without a clock running out (at most three), spends one at the deadline of a clock instead of being answered for (the deadline moves by the extension, the seat is sent the decision again and every seat the bank), keeps the bank when it answers in time, is told the time and the bank that are left when it comes back, starts with an empty bank after a resume, and gets its countdown after the grace and a clock of the active seat only when the settings say so, whatever it holds; with the bank off the table plays as it did before.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::time::Duration;

use common::{Client, FlakyStore, game_id, passive, roomy};
use mtg_engine::decision::DecisionKind;
use mtg_engine::{Decision, GameSetup};
use mtg_protocol::{ClientMessage, DecisionId, ServerMessage};
use mtg_server::{Dwell, GameConfig, GameHandle, Pace, resume_game_with, spawn_game_with};
use mtg_testkit::Scenario;
use mtg_testkit::cards::instant;
use rstest::rstest;

/// The response clock and the extension in milliseconds.
const RESPONSE: u64 = 10_000;
const EXTENSION: u64 = 20_000;
const ACTIVE: u64 = 30_000;

/// Every seat is asked at every window (auto-pass off), a response clock of 10 s, an extension of 20 s and no grace.
fn pace() -> Pace {
    Pace {
        auto_pass: false,
        response: Some(Duration::from_millis(RESPONSE)),
        blocks: Some(Duration::from_secs(45)),
        think: Some(Duration::from_secs(60)),
        active: None,
        extension: Some(Duration::from_millis(EXTENSION)),
        grace: Duration::ZERO,
        dwell: Dwell::OFF,
    }
}

/// What a seat was last sent for its decision.
#[derive(Clone, Debug)]
struct Sent {
    id: DecisionId,
    decision: Decision,
    left: Option<u64>,
    show_after: Option<u64>,
}

struct Table {
    handle: GameHandle,
    store: FlakyStore,
    clients: Vec<Client>,
    /// Everything each seat was sent, in order.
    seen: Vec<Vec<ServerMessage>>,
}

impl Table {
    async fn start(seats: usize, pace: Pace) -> Table {
        Table::start_with(Scenario::new(seats).setup().unwrap(), seats, pace).await
    }

    async fn start_with(setup: GameSetup, seats: usize, pace: Pace) -> Table {
        let store = FlakyStore::new();
        let config = GameConfig { pace, ..roomy() };
        let handle = spawn_game_with(game_id(1), setup, store.clone(), config).unwrap();
        let mut clients = Vec::new();
        for seat in 0..seats {
            clients.push(Client::join(&handle, seat).await);
        }
        for client in &clients {
            client.say(&handle, ClientMessage::Ready).await;
        }
        let mut table = Table {
            handle,
            store,
            clients,
            seen: vec![Vec::new(); seats],
        };
        table.settle().await;
        table
    }

    /// Lets the actor and its timers run until nothing is left to do at this instant, then reads what every seat was asked.
    async fn settle(&mut self) {
        for _ in 0..20 {
            tokio::task::yield_now().await;
            self.handle.status().await.unwrap();
        }
        for (seat, client) in self.clients.iter_mut().enumerate() {
            self.seen[seat].extend(client.drain());
        }
    }

    /// Moves the paused clock by `millis`.
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

    /// The last decision `seat` was asked.
    fn sent(&self, seat: usize) -> Sent {
        self.seen[seat]
            .iter()
            .rev()
            .find_map(|message| match message {
                ServerMessage::Decision {
                    id,
                    decision,
                    deadline_ms,
                    show_after_ms,
                } => Some(Sent {
                    id: *id,
                    decision: decision.clone(),
                    left: *deadline_ms,
                    show_after: *show_after_ms,
                }),
                _ => None,
            })
            .expect("the seat was sent a decision")
    }

    async fn answer(&mut self, seat: usize, asked: &Sent) {
        let answer = passive(&asked.decision);
        let message = ClientMessage::Answer {
            decision: asked.id,
            answer,
        };
        self.clients[seat].say(&self.handle, message).await;
        self.settle().await;
    }

    /// The turn number and the active seat of the latest batch asked.
    fn turn(&self) -> (u32, usize) {
        self.seen[0]
            .iter()
            .rev()
            .find_map(|message| match message {
                ServerMessage::Events(batch) => {
                    Some((batch.table.turn.number, batch.table.turn.active.index()))
                }
                _ => None,
            })
            .unwrap_or((1, 0))
    }

    /// How many times `seat` was sent the decision `id`.
    fn times_sent(&self, seat: usize, id: DecisionId) -> usize {
        let same = |message: &&ServerMessage| matches!(message, ServerMessage::Decision { id: heard, .. } if *heard == id);
        self.seen[seat].iter().filter(same).count()
    }

    /// The bank of every seat as `viewer` knows it from its snapshots, batches and `Bank` messages.
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

    /// The `Bank` messages `viewer` was sent, as (seat, bank).
    fn bank_messages(&self, viewer: usize) -> Vec<(usize, u8)> {
        self.seen[viewer]
            .iter()
            .filter_map(|message| match message {
                ServerMessage::Bank { seat, bank } => Some((seat.index(), *bank)),
                _ => None,
            })
            .collect()
    }

    /// Plays until `done`: a seat for which `slow` says so lets its clock run out, every other answer comes in time.
    async fn play(
        &mut self,
        mut slow: impl FnMut(&Table, usize) -> bool,
        done: impl Fn(&Table) -> bool,
    ) {
        for _ in 0..4000 {
            if done(self) {
                return;
            }
            let seat = self.waiting().await;
            let asked = self.sent(seat);
            if slow(self, seat) {
                self.wait(asked.left.expect("a clock")).await;
            } else {
                self.answer(seat, &asked).await;
            }
        }
        panic!("the game never got there");
    }

    /// The first turn number at which `who` has finished `turns` turns of its own.
    fn after_own_turns(&self, who: usize, turns: u32) -> u32 {
        let seats = u32::try_from(self.seen.len()).unwrap();
        (turns - 1) * seats + u32::try_from(who).unwrap() + 2
    }

    /// Plays in time until the game is in turn `turn` or later and waits for `asked`.
    async fn play_to(&mut self, turn: u32, asked: usize) {
        for _ in 0..4000 {
            let seat = self.waiting().await;
            if self.turn().0 >= turn && seat == asked {
                return;
            }
            let asked = self.sent(seat);
            self.answer(seat, &asked).await;
        }
        panic!("the game never got there");
    }

    /// Plays in time until `who` has finished `turns` turns of its own and is asked in a turn after them.
    async fn play_own_turns(&mut self, who: usize, turns: u32) {
        let turn = self.after_own_turns(who, turns);
        self.play_to(turn, who).await;
    }
}

#[rstest]
#[case::two_turns_are_not_enough(2, 1, 2, 0)]
#[case::three_turns_earn_one(2, 1, 3, 1)]
#[case::the_first_seat_earns_on_its_own_turns(2, 0, 3, 1)]
#[case::three_seats(3, 1, 3, 1)]
#[case::four_seats(4, 2, 3, 1)]
#[case::four_turns_are_still_one(2, 1, 4, 1)]
#[case::six_turns_earn_two(2, 1, 6, 2)]
#[case::the_bank_holds_three(2, 1, 12, 3)]
#[tokio::test(start_paused = true)]
async fn extensions_are_earned_by_own_turns_finished_without_a_clock(
    #[case] seats: usize,
    #[case] who: usize,
    #[case] turns: u32,
    #[case] held: u8,
) {
    let mut table = Table::start(seats, pace()).await;
    table.play_own_turns(who, turns).await;
    let banks = table.banks(0);
    assert_eq!(banks[who], held);
    for viewer in 1..seats {
        assert_eq!(table.banks(viewer), banks, "the bank is public");
    }
    assert!(
        table.bank_messages(0).is_empty(),
        "an earned extension arrives with the batch of the turn"
    );
}

#[rstest]
#[case::before_its_first_turn(1, 7)]
#[case::in_its_own_turn(2, 9)]
#[case::in_another_turn_after_its_own(3, 9)]
#[tokio::test(start_paused = true)]
async fn a_clock_that_runs_out_starts_the_run_again(
    #[case] slow_turn: u32,
    #[case] earned_at: u32,
) {
    // Seat 1's own turns have a clock too, so one can run out in them.
    let settings = Pace {
        active: Some(Duration::from_millis(ACTIVE)),
        ..pace()
    };
    let mut table = Table::start(2, settings).await;
    let mut ran_out = false;
    table
        .play(
            |table, seat| {
                let slow = seat == 1 && !ran_out && table.turn().0 >= slow_turn;
                ran_out |= slow;
                slow
            },
            |table| table.turn().0 >= earned_at - 1,
        )
        .await;
    assert_eq!(table.banks(1)[1], 0, "one turn short");
    table
        .play(|_, _| false, |table| table.turn().0 >= earned_at)
        .await;
    assert_eq!(table.banks(1)[1], 1);
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
#[tokio::test(start_paused = true)]
async fn a_clock_that_runs_out_spends_an_extension_and_moves_the_deadline(#[case] seats: usize) {
    let mut table = Table::start(seats, pace()).await;
    table.play_own_turns(1, 3).await;
    assert_eq!(table.banks(0)[1], 1);
    let seat = table.waiting().await;
    assert_ne!(seat, 0, "the seat under test is waited for");
    let first = table.sent(seat);
    let events = table.events().await;
    assert_eq!(first.left, Some(RESPONSE));

    table.wait(RESPONSE).await;

    let again = table.sent(seat);
    assert_eq!(again.id, first.id, "the same decision");
    assert_eq!(again.decision, first.decision);
    assert_eq!(
        again.left,
        Some(EXTENSION),
        "the deadline moved by the extension"
    );
    assert_eq!(again.show_after, None, "the countdown is already showing");
    assert_eq!(table.events().await, events, "the table did not answer");
    assert_eq!(table.waiting().await, seat);
    for viewer in 0..seats {
        assert_eq!(
            table.bank_messages(viewer),
            [(seat, 0)],
            "every seat is told the bank"
        );
        assert_eq!(table.banks(viewer)[seat], 0);
    }

    table.wait(EXTENSION - 1).await;
    assert_eq!(
        table.events().await,
        events,
        "an extension is a whole length"
    );
    table.wait(1).await;
    assert!(
        table.events().await > events,
        "the empty bank lets the table answer"
    );
    assert_eq!(table.bank_messages(0).len(), 1, "no second extension");
}

#[tokio::test(start_paused = true)]
async fn a_bank_of_three_spends_three_times_and_then_the_table_answers() {
    let mut table = Table::start(2, pace()).await;
    table.play_own_turns(1, 9).await;
    assert_eq!(table.banks(0)[1], 3);
    let seat = table.waiting().await;
    let id = table.sent(seat).id;
    let events = table.events().await;
    let mut deadlines = Vec::new();

    table.wait(RESPONSE).await;
    for _ in 0..3 {
        let asked = table.sent(seat);
        assert_eq!(asked.id, id);
        deadlines.push(asked.left);
        table.wait(EXTENSION).await;
    }

    assert_eq!(deadlines, [Some(EXTENSION); 3]);
    assert_eq!(
        table.bank_messages(0),
        [(seat, 2), (seat, 1), (seat, 0)],
        "one message per extension"
    );
    assert!(table.events().await > events, "then the table answers");
    assert_eq!(
        table.times_sent(seat, id),
        4,
        "sent once and again for each extension"
    );
}

#[tokio::test(start_paused = true)]
async fn answering_in_time_keeps_the_bank() {
    let mut table = Table::start(2, pace()).await;
    table.play_own_turns(1, 3).await;
    let turn = table.turn().0;
    table
        .play(|_, _| false, |table| table.turn().0 >= turn + 2)
        .await;
    assert_eq!(table.banks(0)[1], 1);
    assert_eq!(table.banks(1)[1], 1);
    assert_eq!(table.bank_messages(0), []);
}

#[tokio::test(start_paused = true)]
async fn without_an_extension_setting_nothing_is_earned_and_the_clock_answers_at_the_deadline() {
    let off = Pace {
        extension: None,
        ..pace()
    };
    let mut table = Table::start(2, off).await;
    table.play_own_turns(1, 4).await;
    assert_eq!(table.banks(0), [0, 0]);
    let seat = table.waiting().await;
    let first = table.sent(seat);
    let events = table.events().await;

    table.wait(RESPONSE - 1).await;
    assert_eq!(table.events().await, events);
    table.wait(1).await;

    assert!(
        table.events().await > events,
        "the table answers at the deadline"
    );
    assert_eq!(
        table.times_sent(seat, first.id),
        1,
        "no decision is sent again"
    );
    for viewer in 0..2 {
        assert_eq!(table.bank_messages(viewer), []);
        assert!(table.seen[viewer].iter().all(|message| !matches!(
            message,
            ServerMessage::Decision {
                show_after_ms: Some(_),
                ..
            }
        )));
    }
}

#[rstest]
#[case::no_clock_for_the_active_seat(None)]
#[case::a_clock_for_the_active_seat(Some(ACTIVE))]
#[tokio::test(start_paused = true)]
async fn the_active_seat_has_a_clock_only_when_the_settings_give_one(#[case] active: Option<u64>) {
    let settings = Pace {
        active: active.map(Duration::from_millis),
        grace: Duration::from_secs(3),
        ..pace()
    };
    let mut table = Table::start(2, settings).await;
    assert_eq!(table.waiting().await, 0, "seat 0 starts");
    let asked = table.sent(0);
    assert!(matches!(asked.decision.kind, DecisionKind::Priority(_)));
    assert_eq!(asked.left, active);
    assert_eq!(asked.show_after, active.map(|_| 3_000));
    let events = table.events().await;

    table.wait(active.unwrap_or(RESPONSE) + 60_000).await;

    if active.is_some() {
        assert!(
            table.events().await > events,
            "the table passes for the active seat"
        );
    } else {
        assert_eq!(
            table.events().await,
            events,
            "the active seat takes as long as it takes"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn the_clock_of_the_active_seat_takes_an_extension_too() {
    let settings = Pace {
        active: Some(Duration::from_millis(ACTIVE)),
        ..pace()
    };
    let mut table = Table::start(2, settings).await;
    table.play_to(7, 0).await;
    assert_eq!(table.banks(1)[0], 1);
    assert_eq!(table.waiting().await, 0, "seat 0's fourth turn begins");
    let first = table.sent(0);
    assert_eq!(first.left, Some(ACTIVE));
    let events = table.events().await;

    table.wait(ACTIVE).await;

    let again = table.sent(0);
    assert_eq!((again.id, again.left), (first.id, Some(EXTENSION)));
    assert_eq!(table.events().await, events);
    assert_eq!(table.bank_messages(1), [(0, 0)]);
    table.wait(EXTENSION).await;
    assert!(table.events().await > events);
}

#[rstest]
#[case::no_grace(0, Some(RESPONSE), None)]
#[case::three_seconds(3, Some(RESPONSE), Some(3_000))]
#[case::nine_seconds(9, Some(RESPONSE), Some(9_000))]
#[case::a_grace_as_long_as_the_clock(10, Some(RESPONSE), None)]
#[case::a_grace_longer_than_the_clock(30, Some(RESPONSE), None)]
#[tokio::test(start_paused = true)]
async fn the_countdown_shows_after_the_grace_and_the_clock_runs_from_the_start(
    #[case] grace: u64,
    #[case] left: Option<u64>,
    #[case] show_after: Option<u64>,
) {
    let settings = Pace {
        grace: Duration::from_secs(grace),
        ..pace()
    };
    let mut table = Table::start(2, settings).await;
    table.answer(0, &table.sent(0)).await;
    assert_eq!(table.waiting().await, 1);
    let asked = table.sent(1);
    assert_eq!((asked.left, asked.show_after), (left, show_after));
    let events = table.events().await;
    table.wait(RESPONSE).await;
    assert!(
        table.events().await > events,
        "the clock ran from the start, the grace included"
    );
}

#[tokio::test(start_paused = true)]
async fn a_seat_that_comes_back_is_told_what_is_left_of_the_grace_the_clock_and_the_bank() {
    let settings = Pace {
        grace: Duration::from_secs(3),
        ..pace()
    };
    let mut table = Table::start(2, settings).await;
    table.play_own_turns(1, 3).await;
    let seat = table.waiting().await;
    let first = table.sent(seat);
    assert_eq!(
        (first.left, first.show_after),
        (Some(RESPONSE), Some(3_000))
    );

    table.wait(1_000).await;
    table.clients[seat]
        .say(&table.handle, ClientMessage::Resync)
        .await;
    table.settle().await;
    let back = table.sent(seat);
    assert_eq!(back.id, first.id);
    assert_eq!((back.left, back.show_after), (Some(9_000), Some(2_000)));
    assert_eq!(table.banks(seat)[1], 1);

    table.wait(9_000).await;
    assert_eq!(table.sent(seat).left, Some(EXTENSION));
    table.wait(5_000).await;
    table.clients[seat]
        .say(&table.handle, ClientMessage::Resync)
        .await;
    table.settle().await;
    let back = table.sent(seat);
    assert_eq!(back.id, first.id);
    assert_eq!(
        (back.left, back.show_after),
        (Some(EXTENSION - 5_000), None),
        "the deadline moved and the countdown shows at once"
    );
    for viewer in 0..2 {
        assert_eq!(
            table.banks(viewer)[1],
            0,
            "the spent extension is told to every seat"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_resumed_game_starts_every_bank_empty() {
    let mut table = Table::start(2, pace()).await;
    table.play_own_turns(1, 3).await;
    assert_eq!(table.banks(0)[1], 1);
    let events = table.events().await;
    let Table { handle, store, .. } = table;
    drop(handle);

    let setup = Scenario::new(2).setup().unwrap();
    let config = GameConfig {
        pace: pace(),
        ..roomy()
    };
    let second = resume_game_with(store, game_id(1), setup, config)
        .await
        .unwrap();
    assert_eq!(second.status().await.unwrap().events, events);
    let mut back = Client::join(&second, 1).await;
    tokio::task::yield_now().await;
    second.status().await.unwrap();
    let messages = back.drain();
    let banks: Vec<Vec<u8>> = messages
        .iter()
        .filter_map(|message| match message {
            ServerMessage::Snapshot { state, .. } => {
                Some(state.table.players.iter().map(|p| p.bank).collect())
            }
            _ => None,
        })
        .collect();
    assert_eq!(banks, [vec![0, 0]], "the bank is kept in memory only");
}

#[tokio::test(start_paused = true)]
async fn the_clock_fields_are_the_same_for_a_seat_that_can_act_and_one_that_cannot() {
    let setup = Scenario::new(3)
        .hand(1, [instant("Zap", 0, &[])])
        .setup()
        .unwrap();
    let settings = Pace {
        grace: Duration::from_secs(3),
        ..pace()
    };
    let mut table = Table::start_with(setup, 3, settings).await;
    let mut told = Vec::new();
    for _ in 0..3 {
        let seat = table.waiting().await;
        let asked = table.sent(seat);
        if let DecisionKind::Priority(options) = &asked.decision.kind
            && seat != 0
        {
            told.push((options.can_act, asked.left, asked.show_after));
        }
        table.answer(seat, &asked).await;
    }
    assert_eq!(
        told,
        [
            (true, Some(RESPONSE), Some(3_000)),
            (false, Some(RESPONSE), Some(3_000))
        ],
        "a seat with a spell in hand and one with nothing are told the same clock"
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_save_earns_nothing_and_the_retry_earns_the_extension() {
    let mut table = Table::start(2, pace()).await;
    table.play(|_, _| false, |table| table.turn().0 >= 6).await;
    // Every answer from the start of turn 6 is refused once by the store before it is accepted.
    while table.turn().0 < 7 {
        let seat = table.waiting().await;
        let asked = table.sent(seat);
        let before = table.banks(1);
        table.store.fail(true);
        table.answer(seat, &asked).await;
        table.store.fail(false);
        // A snapshot says what the table holds now.
        let resync = ClientMessage::Resync;
        table.clients[1].say(&table.handle, resync).await;
        table.settle().await;
        assert_eq!(
            table.banks(1),
            before,
            "a move that was not saved earns nothing"
        );
        table.answer(seat, &asked).await;
    }
    assert_eq!(table.banks(0)[1], 1, "the extension is earned once");
}
