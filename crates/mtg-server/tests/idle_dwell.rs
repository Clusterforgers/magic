//! Goal: a seat that cannot act at a window of the default table waits a dwell and is then passed for as a batch of its own, while every other pass stays at once and a seat that can act, holds a stop, is in Full Control or yields is not made to wait; a round of idle seats waits one dwell and then short staggers within a cap; a seat's own command ends its dwell and the rules decide again while anything else leaves it alone; a failed save at the end of a dwell is retried; and the wait is wall clock only, so bot games and resumed games write the log of the game without it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::{
    Bot, FlakyStore, Tape, game_id, kind, ms, passive, play_bots, reference_windows, roomy, seat,
    table_setup,
};
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, GameSetup};
use mtg_protocol::{
    ClientMessage, DecisionId, ErrorCode, FullControlMode, ServerMessage, StopSide, YieldUntil,
};
use mtg_server::{
    Dwell, EventStore, GameConfig, GameHandle, Pace, resume_game_with, spawn_game_with,
};
use mtg_testkit::Scenario;
use mtg_testkit::cards::{free_creature, giant};
use rstest::rstest;
use tokio::time::Instant;

/// Seats whose libraries hold nothing either can cast or play, so a seat can act only with what a test puts in its hand.
fn quiet(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for n in 0..seats {
        scenario = scenario.library(n, vec![giant(); 30]);
    }
    scenario
}

/// [`quiet`] with a creature that costs nothing in the hand of seat 0, which can then act at its main phase.
fn with_a_play(seats: usize) -> GameSetup {
    quiet(seats)
        .hand(0, [free_creature("Spare Bear", 2, 2)])
        .setup()
        .unwrap()
}

/// The default windows, no clocks and this dwell.
fn config(dwell: Dwell) -> GameConfig {
    GameConfig {
        pace: Pace {
            auto_pass: true,
            response: None,
            blocks: None,
            dwell,
        },
        ..roomy()
    }
}

/// A game on paused time whose seats only listen and say what a test makes them say.
struct Table {
    handle: GameHandle,
    store: FlakyStore,
    tapes: Vec<Tape>,
    /// When the table opened: arrival times count from here.
    since: Instant,
}

impl Table {
    /// Seats `seats` listeners in the lobby of a game made from `setup`.
    async fn lobby(setup: GameSetup, config: GameConfig, seats: usize) -> Table {
        let since = Instant::now();
        let store = FlakyStore::new();
        let handle = spawn_game_with(game_id(1), setup, store.clone(), config).unwrap();
        let mut tapes = Vec::new();
        for n in 0..seats {
            let inbox = handle.join(seat(n)).await.unwrap();
            tapes.push(Tape::listen(inbox, since));
        }
        Table {
            handle,
            store,
            tapes,
            since,
        }
    }

    /// Seats listeners and has every one of them say it is ready.
    async fn start(setup: GameSetup, config: GameConfig, seats: usize) -> Table {
        let table = Table::lobby(setup, config, seats).await;
        for n in 0..seats {
            table.send(n, ClientMessage::Ready).await;
        }
        table.settle().await;
        table
    }

    async fn send(&self, from: usize, message: ClientMessage) {
        self.handle.send(seat(from), message).await.unwrap();
    }

    /// Lets the game run until `millis` after the table opened, and the actor and the listeners catch up.
    async fn run_to(&self, millis: u64) {
        tokio::time::sleep_until(self.since + Duration::from_millis(millis)).await;
        self.settle().await;
    }

    /// Lets the actor deal with everything sent so far and the listeners write it down.
    async fn settle(&self) {
        self.handle.status().await.unwrap();
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
    }

    /// Milliseconds since the table opened.
    fn now(&self) -> u64 {
        ms(self.since.elapsed())
    }

    /// The newest decision `from` was sent.
    fn decision(&self, from: usize) -> (DecisionId, mtg_engine::Decision) {
        self.tapes[from]
            .last_decision()
            .expect("the seat was sent a decision")
    }

    /// Answers the newest decision of `from` with `answer`.
    async fn answer(&self, from: usize, answer: Answer) {
        let (decision, _) = self.decision(from);
        self.send(from, ClientMessage::Answer { decision, answer })
            .await;
        self.settle().await;
    }

    /// Seat `from` casts the first spell it can and pays for it.
    async fn cast(&self, from: usize) {
        let (_, decision) = self.decision(from);
        let DecisionKind::Priority(options) = &decision.kind else {
            panic!("expected priority, got {decision:?}");
        };
        let spell = options.castable[0];
        self.answer(from, Answer::Priority(PriorityAction::CastSpell(spell)))
            .await;
        self.answer(from, Answer::PayMana(PayChoice::Auto)).await;
    }

    /// When the batches `from` was sent arrived, in milliseconds since the table opened.
    fn batch_times(&self, from: usize) -> Vec<u64> {
        self.tapes[from]
            .batches()
            .iter()
            .map(|(at, _)| ms(*at))
            .collect()
    }

    /// The turn and step the table was in as each batch `from` was sent left it.
    fn places(&self, from: usize) -> Vec<(u32, Step)> {
        self.tapes[from]
            .batches()
            .iter()
            .map(|(_, batch)| (batch.table.turn.number, batch.table.turn.step))
            .collect()
    }

    /// Decisions sent to seat `from`, as the time each arrived.
    fn decision_times(&self, from: usize) -> Vec<u64> {
        self.tapes[from]
            .decisions()
            .iter()
            .map(|(at, _, _)| ms(*at))
            .collect()
    }

    /// The events written so far.
    async fn log(&self) -> Vec<mtg_engine::Event> {
        self.store.load(game_id(1)).await.unwrap()
    }
}

fn own(step: Step) -> ClientMessage {
    ClientMessage::SetStop {
        side: StopSide::Own,
        step,
        on: true,
        keep: false,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The dwell itself

#[tokio::test(start_paused = true)]
async fn an_idle_main_window_waits_its_dwell_and_the_pass_is_a_batch_of_its_own() {
    let table = Table::start(quiet(2).setup().unwrap(), config(Dwell::fixed(600)), 2).await;
    table.run_to(2_500).await;

    for n in 0..2 {
        assert_eq!(
            table.batch_times(n),
            [0, 600, 1200, 1800, 2400],
            "seat {n} hears of every pass when its dwell is over"
        );
        // The steps in between (upkeep, draw, combat, end) pass at once, inside the batch of the pass before them.
        assert_eq!(
            table.places(n),
            [
                (1, Step::Main1),
                (1, Step::Main2),
                (2, Step::Main1),
                (2, Step::Main2),
                (3, Step::Main1)
            ]
        );
        assert!(
            table.tapes[n].decisions().is_empty(),
            "nobody is asked for a window it cannot use"
        );
        let mut next = 0;
        for (_, batch) in table.tapes[n].batches() {
            assert_eq!(batch.from_seq, next, "the batches follow one another");
            next += u64::try_from(batch.events.len()).unwrap();
        }
        assert_eq!(table.log().await.len(), usize::try_from(next).unwrap());
    }
    // The game waits on the seat that dwells although the seat was told nothing.
    let status = table.handle.status().await.unwrap();
    assert_eq!(status.waiting_on, Some(seat(0)));
}

#[tokio::test(start_paused = true)]
async fn a_dwell_drawn_between_300_and_900_stays_inside_that_range() {
    let table = Table::start(quiet(2).setup().unwrap(), config(Dwell::DEFAULT), 2).await;
    table.run_to(30_000).await;

    // The game stops at the first discard, when a seat that never plays holds eight cards: about 33 windows.
    let times = table.batch_times(1);
    assert!(times.len() >= 25, "{} batches in 30 s", times.len());
    let gaps: Vec<u64> = times.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(
        gaps.iter().all(|gap| (300..=900).contains(gap)),
        "every wait is one dwell: {gaps:?}"
    );
    assert!(gaps.iter().any(|gap| *gap < 600), "short ones: {gaps:?}");
    assert!(gaps.iter().any(|gap| *gap > 600), "and long ones: {gaps:?}");
}

#[tokio::test(start_paused = true)]
async fn a_seat_that_can_act_is_asked_at_once_and_nobody_waits_for_it() {
    let table = Table::start(with_a_play(2), config(Dwell::fixed(600)), 2).await;
    table.run_to(2_000).await;

    assert_eq!(table.decision_times(0), [0], "asked the moment it can act");
    assert_eq!(table.batch_times(1), [0], "and the table waits for it");

    table.answer(0, passive(&table.decision(0).1)).await;
    assert_eq!(
        table.batch_times(1),
        [0, 2_000],
        "its pass reaches the other seat when it is made, with no dwell added"
    );
    // Seat 0 can still act in its second main phase and is asked again, at once.
    assert_eq!(table.decision_times(0), [0, 2_000]);
}

#[rstest]
#[case::a_stop(Some(own(Step::Main1)), None)]
#[case::the_next_window(
    None,
    Some(ClientMessage::FullControl { mode: FullControlMode::NextWindow })
)]
#[case::full_control_held(
    None,
    Some(ClientMessage::FullControl { mode: FullControlMode::Held })
)]
#[case::auto_pass_off(None, Some(ClientMessage::AutoPass { enabled: false }))]
#[tokio::test(start_paused = true)]
async fn a_stop_or_full_control_window_is_the_seats_own_to_answer_with_no_dwell(
    #[case] stop: Option<ClientMessage>,
    #[case] mode: Option<ClientMessage>,
) {
    let table = Table::lobby(quiet(2).setup().unwrap(), config(Dwell::fixed(600)), 2).await;
    for message in stop.into_iter().chain(mode) {
        table.send(0, message).await;
    }
    table.send(0, ClientMessage::Ready).await;
    table.send(1, ClientMessage::Ready).await;
    table.run_to(5_000).await;

    assert_eq!(
        table.decision_times(0),
        [0],
        "asked at once, though it cannot act"
    );
    assert_eq!(
        table.batch_times(1),
        [0],
        "and the table waits for its answer"
    );
}

#[tokio::test(start_paused = true)]
async fn a_yield_is_the_seats_choice_to_hurry_and_waits_for_nothing() {
    let table = Table::start(with_a_play(2), config(Dwell::fixed(600)), 2).await;
    table.run_to(1_000).await;
    table
        .send(
            0,
            ClientMessage::Yield {
                until: YieldUntil::EndOfTurn,
            },
        )
        .await;
    table.run_to(3_000).await;

    // The rest of seat 0's turn passes at once, though seat 0 could act in its second main phase; seat 1's two
    // windows of its own turn are idle and wait, and seat 0 is asked again when its next turn begins.
    assert_eq!(table.batch_times(1), [0, 1_000, 1_600, 2_200]);
    assert_eq!(
        table.places(1),
        [
            (1, Step::Main1),
            (2, Step::Main1),
            (2, Step::Main2),
            (3, Step::Main1)
        ]
    );
    assert_eq!(table.decision_times(0), [0, 2_200]);
}

#[tokio::test(start_paused = true)]
async fn a_window_that_would_run_a_response_clock_is_passed_after_its_dwell_and_not_at_the_clock() {
    // Seat 1 is not the active player, so a window it was sent would run a 10 s clock; this one it is not sent.
    let table = Table::start(
        with_a_play(2),
        GameConfig {
            pace: Pace {
                response: Some(Duration::from_secs(10)),
                ..config(Dwell::fixed(600)).pace
            },
            ..config(Dwell::fixed(600))
        },
        2,
    )
    .await;
    table.cast(0).await;
    table.run_to(60_000).await;
    let status = table.handle.status().await.unwrap();
    assert!(status.events > 50, "the game went on: {status:?}");
    let gaps: Vec<u64> = table
        .batch_times(1)
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    assert!(
        gaps.iter().all(|gap| *gap <= 600),
        "no pass waits for a clock: {gaps:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Rounds

#[rstest]
#[case::four_seats(4)]
#[case::six_seats(6)]
#[tokio::test(start_paused = true)]
async fn idle_seats_in_one_round_wait_one_dwell_then_staggers_and_the_round_stops_at_its_cap(
    #[case] seats: usize,
) {
    let table = Table::start(with_a_play(seats), config(Dwell::fixed(900)), seats).await;
    table.cast(0).await;
    let cast_at = table.now();
    table.run_to(cast_at + 10_000).await;

    // When each pass after the cast reached the last seat: seat 0 first, then every other seat, then seat 0 once more
    // when the spell has resolved (those are the `seats + 1` passes of the round), then the pass of the next step.
    let arrivals: Vec<u64> = table
        .batch_times(seats - 1)
        .into_iter()
        .filter(|at| *at > cast_at)
        .map(|at| at - cast_at)
        .collect();
    assert_eq!(
        arrivals[0], 900,
        "the first idle seat of the round waits a dwell"
    );
    let gaps: Vec<u64> = arrivals[..=seats]
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    assert!(
        gaps.iter().all(|gap| *gap <= 250),
        "the others wait a stagger of at most 250: {gaps:?}"
    );
    assert_eq!(
        arrivals[seats], 1_200,
        "and the whole round waits 1200 at most: {arrivals:?}"
    );
    // Four staggers of 80 or more use the 300 ms left after a dwell of 900 up, so each one after the fourth is 0.
    assert!(
        gaps.iter().skip(4).all(|gap| *gap == 0),
        "past the cap the passes follow at once: {gaps:?}"
    );
    assert_eq!(
        arrivals[seats + 1],
        1_200 + 900,
        "the next step is a round of its own and waits a dwell: {arrivals:?}"
    );
    assert!(
        table.tapes.iter().all(|tape| tape
            .decisions()
            .iter()
            .all(|(_, _, decision)| decision.player == seat(0))),
        "only seat 0 was ever sent anything to answer"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// What a seat does during its dwell

/// Seat 0 dwells at its first main phase from 0 to 600; at 200 it, or seat 1, sends `message`. Returns the table at 700.
async fn disturbed(from: usize, message: ClientMessage, seats: usize) -> Table {
    let table = Table::start(
        quiet(seats).setup().unwrap(),
        config(Dwell::fixed(600)),
        seats,
    )
    .await;
    table.run_to(200).await;
    assert_eq!(table.batch_times(0), [0], "still dwelling at 200");
    table.send(from, message).await;
    table.run_to(700).await;
    table
}

#[rstest]
#[case::a_stop_at_this_window(own(Step::Main1))]
#[case::the_next_window(ClientMessage::FullControl { mode: FullControlMode::NextWindow })]
#[case::full_control_held(ClientMessage::FullControl { mode: FullControlMode::Held })]
#[case::auto_pass_off(ClientMessage::AutoPass { enabled: false })]
#[tokio::test(start_paused = true)]
async fn a_seat_that_asks_for_the_window_during_its_dwell_is_sent_it_at_once(
    #[case] message: ClientMessage,
) {
    let table = disturbed(0, message, 2).await;
    assert_eq!(
        table.decision_times(0),
        [200],
        "the dwell is over the moment the seat asks"
    );
    assert_eq!(
        table.batch_times(1),
        [0],
        "and the table waits for its answer"
    );
    // The notice of the change comes first, as it does for a decision that was sent in the first place.
    let kinds = table.tapes[0].kinds();
    assert_eq!(kinds.last().map(String::as_str), Some("Decision"));
    assert!(
        matches!(kinds[kinds.len() - 2].as_str(), "Stops" | "FullControl"),
        "{kinds:?}"
    );

    // It is a decision like any other: the seat answers it and the game goes on.
    table.answer(0, passive(&table.decision(0).1)).await;
    assert_eq!(table.batch_times(1).len(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_yield_during_the_dwell_passes_at_once() {
    let table = disturbed(
        0,
        ClientMessage::Yield {
            until: YieldUntil::EndOfTurn,
        },
        2,
    )
    .await;
    // The pass came at 200, with everything the yield passes for; seat 1's idle window of the next turn waits.
    assert_eq!(table.batch_times(1), [0, 200]);
    assert_eq!(table.places(1)[1], (2, Step::Main1));
    assert_eq!(table.decision_times(0), Vec::<u64>::new());
    table.run_to(900).await;
    assert_eq!(table.batch_times(1), [0, 200, 800]);
}

#[tokio::test(start_paused = true)]
async fn a_concession_during_the_dwell_ends_the_game_and_the_dwell_with_it() {
    let table = disturbed(0, ClientMessage::Concede, 2).await;
    assert_eq!(table.batch_times(1), [0, 200]);
    assert!(table.tapes[1].kinds().contains(&"GameOver".to_owned()));
    table.run_to(5_000).await;
    assert_eq!(table.batch_times(1), [0, 200], "no pass follows");
}

#[rstest]
#[case::chat_from_the_other_seat(1, ClientMessage::Chat { text: "hello".to_owned() })]
#[case::a_stop_at_another_step(0, own(Step::Main2))]
#[case::a_stop_of_another_seat_at_this_step(1, own(Step::Main1))]
#[case::a_repeated_resync(0, ClientMessage::Resync)]
#[case::a_guessed_decision_id(
    0,
    ClientMessage::Answer {
        decision: DecisionId(1),
        answer: Answer::Priority(PriorityAction::Pass),
    }
)]
#[case::a_guessed_decision_id_of_the_other_seat(
    1,
    ClientMessage::Answer {
        decision: DecisionId(2),
        answer: Answer::Priority(PriorityAction::Pass),
    }
)]
#[tokio::test(start_paused = true)]
async fn nothing_else_moves_a_dwell(#[case] from: usize, #[case] message: ClientMessage) {
    let table = disturbed(from, message, 2).await;
    assert_eq!(
        table.batch_times(1),
        [0, 600],
        "the pass comes when the dwell, drawn at the start, is over"
    );
    assert!(
        table.tapes[0]
            .decisions()
            .iter()
            .all(|(at, _, _)| ms(*at) >= 600),
        "and the seat was never sent the window: {:?}",
        table.tapes[0].kinds()
    );
}

#[tokio::test(start_paused = true)]
async fn a_resync_during_the_dwell_hands_over_a_snapshot_and_no_decision() {
    let table = disturbed(0, ClientMessage::Resync, 2).await;
    let kinds = table.tapes[0].kinds();
    assert_eq!(
        kinds.iter().filter(|k| *k == "Snapshot").count(),
        1,
        "{kinds:?}"
    );
    assert!(!kinds.contains(&"Decision".to_owned()), "{kinds:?}");
}

#[tokio::test(start_paused = true)]
async fn an_answer_for_a_decision_nobody_was_sent_is_stale_and_changes_nothing() {
    let table = disturbed(
        0,
        ClientMessage::Answer {
            decision: DecisionId(1),
            answer: Answer::Priority(PriorityAction::Pass),
        },
        2,
    )
    .await;
    let errors: Vec<ErrorCode> = table.tapes[0]
        .heard()
        .into_iter()
        .filter_map(|(_, message)| match message {
            ServerMessage::Error { code, .. } => Some(code),
            _ => None,
        })
        .collect();
    assert_eq!(errors, [ErrorCode::StaleDecision]);
}

#[tokio::test(start_paused = true)]
async fn the_concession_of_a_third_seat_leaves_the_dwell_of_the_first_alone() {
    let table = disturbed(2, ClientMessage::Concede, 3).await;
    assert_eq!(
        table.batch_times(1),
        [0, 200, 600],
        "the concession is a batch of its own; seat 0 is passed for when its dwell is over"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// A pass that cannot be saved

#[tokio::test(start_paused = true)]
async fn a_pass_that_cannot_be_saved_leaves_the_game_as_it_was_and_is_tried_again() {
    let table = Table::start(quiet(2).setup().unwrap(), config(Dwell::fixed(600)), 2).await;
    table.run_to(100).await;
    let opening = table.log().await;
    table.store.fail(true);
    table.run_to(700).await;

    assert_eq!(table.log().await, opening, "nothing was written");
    assert_eq!(table.batch_times(1), [0], "and nobody was told of a pass");
    let errors: Vec<ErrorCode> = table.tapes[0]
        .heard()
        .into_iter()
        .filter_map(|(_, message)| match message {
            ServerMessage::Error { code, .. } => Some(code),
            _ => None,
        })
        .collect();
    assert_eq!(errors, [ErrorCode::Internal]);
    assert_eq!(
        table.handle.status().await.unwrap().waiting_on,
        Some(seat(0))
    );

    table.store.fail(false);
    table.run_to(1_700).await;
    assert_eq!(
        table.batch_times(1)[..2],
        [0, 1_600],
        "the pass is tried again a second after it failed (at 600 + 1000), as for a clock"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Wall clock only

#[rstest]
#[tokio::test(start_paused = true)]
async fn bot_games_with_a_dwell_write_the_log_of_the_game_without_one(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2)] seed: u64,
) {
    let setup = table_setup(players, seed);
    let store = FlakyStore::new();
    let handle = spawn_game_with(
        game_id(1),
        setup.clone(),
        store.clone(),
        config(Dwell::between(20, 90)),
    )
    .unwrap();
    let mut bots = Vec::new();
    for n in 0..players {
        bots.push(Bot::join(&handle, n, seed).await);
    }
    let reports = play_bots(&handle, bots).await;

    let straight = reference_windows(setup, seed, true, false);
    assert_eq!(
        store.inner().load(game_id(1)).await.unwrap(),
        straight.events
    );
    assert_eq!(
        handle.status().await.unwrap().state_hash,
        Some(straight.final_hash)
    );
    for report in &reports {
        assert_eq!(report.result, straight.result);
        assert_eq!(report.errors, [], "{:?}", report.seat);
        assert_eq!(report.idle_decisions, 0, "{:?}", report.seat);
    }
}

#[tokio::test(start_paused = true)]
async fn a_resumed_game_starts_its_dwell_afresh_and_writes_the_same_log() {
    let setup = quiet(2).setup().unwrap();
    let whole = Table::start(setup.clone(), config(Dwell::fixed(600)), 2).await;
    whole.run_to(2_500).await;
    let uninterrupted = whole.log().await;
    assert_eq!(whole.batch_times(0).len(), 5);

    let first = Table::start(setup.clone(), config(Dwell::fixed(600)), 2).await;
    first.run_to(1_300).await;
    assert_eq!(first.batch_times(0), [0, 600, 1200]);
    let log_at_the_break = first.log().await;
    let store = first.store.clone();
    // Everything of the first run goes: the game ends with its last handle, and its dwell with it.
    drop(first);
    tokio::task::yield_now().await;

    let resumed_at = Instant::now();
    let handle = resume_game_with(store.clone(), game_id(1), setup, config(Dwell::fixed(600)))
        .await
        .unwrap();
    let tapes: Vec<Tape> = {
        let mut tapes = Vec::new();
        for n in 0..2 {
            let inbox = handle.join(seat(n)).await.unwrap();
            tapes.push(Tape::listen(inbox, resumed_at));
        }
        tapes
    };
    tokio::time::sleep_until(resumed_at + Duration::from_millis(1_300)).await;
    handle.status().await.unwrap();
    tokio::task::yield_now().await;

    // The wait left over from before is not remembered: a full dwell from the resume, then one per window.
    let times: Vec<u64> = tapes[1].batches().iter().map(|(at, _)| ms(*at)).collect();
    assert_eq!(times, [600, 1_200], "after the snapshot, one batch a dwell");
    let log = store.load(game_id(1)).await.unwrap();
    assert!(log.len() > log_at_the_break.len());
    assert_eq!(log, uninterrupted[..log.len()]);
    assert_eq!(kind(&tapes[1].heard()[0].1), "Welcome");
}
