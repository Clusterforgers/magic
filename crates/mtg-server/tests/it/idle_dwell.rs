//! Goal: a seat that cannot act at a window where it waits (the declare attackers and declare blockers steps, a spell on the stack) waits a dwell and is then passed for as a batch of its own, while its own main phase with an empty stack and every other pass stay at once, and a seat that can act, holds a stop, is in Full Control or yields is not made to wait; every idle seat of a table waits a dwell of its own, the later ones no sooner for the seats before them; a seat's own command ends its dwell and the rules decide again while anything else leaves it alone; a failed save at the end of a dwell is retried; and the wait is wall clock only, so bot games and resumed games write the log of the game without it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::time::Duration;

use common::{
    Bot, FlakyStore, Tape, game_id, kind, ms, passive, play_bots, reference_windows, roomy, seat,
    table_setup,
};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, GameSetup};
use mtg_protocol::{
    ClientMessage, DecisionId, ErrorCode, FullControlMode, ServerMessage, StopSide, YieldUntil,
};
use mtg_server::{
    Dwell, EventStore, GameConfig, GameHandle, Pace, resume_game_with, spawn_game_with,
};
use mtg_testkit::cards::{forest, free_creature, giant};
use mtg_testkit::{Scenario, ready};
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

/// A library that opens with a Forest and goes on with giants: its owner can play a land the turn it draws it.
fn forest_first() -> Vec<std::sync::Arc<mtg_cards::CardDefinition>> {
    let mut cards = vec![forest()];
    cards.extend(vec![giant(); 29]);
    cards
}

/// [`quiet`] with a creature that costs nothing in the hand of seat 0, which can then act at its main phase.
fn with_a_play(seats: usize) -> GameSetup {
    quiet(seats)
        .hand(0, [free_creature("Spare Bear", 2, 2)])
        .setup()
        .unwrap()
}

/// [`with_a_play`] where seat 1 draws a Forest on its first turn, so that the game stops there for a seat that can act.
fn with_a_play_and_a_land() -> GameSetup {
    quiet(2)
        .library(1, forest_first())
        .hand(0, [free_creature("Spare Bear", 2, 2)])
        .setup()
        .unwrap()
}

/// Seat 0 is about to attack seat 1 with a creature: the declare attackers step of turn 2 opens on its declaration.
/// Nobody holds a card, so no seat can act in the windows that follow.
fn combat(seats: usize) -> GameSetup {
    quiet(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(free_creature("Raider", 2, 2))])
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
            think: None,
            active: None,
            extension: None,
            grace: std::time::Duration::ZERO,
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

    /// Seat 0 attacks seat 1 with every creature that can.
    async fn attack(&self) {
        let (_, decision) = self.decision(0);
        let DecisionKind::DeclareAttackers(options) = &decision.kind else {
            panic!("expected the attack, got {decision:?}");
        };
        let attackers = options
            .attackers
            .iter()
            .map(|attacker| (attacker.object, AttackTarget::Player(seat(1))))
            .collect();
        self.answer(0, Answer::DeclareAttackers(attackers)).await;
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

/// The errors `from` was sent.
fn errors(table: &Table, from: usize) -> Vec<ErrorCode> {
    table.tapes[from]
        .heard()
        .into_iter()
        .filter_map(|(_, message)| match message {
            ServerMessage::Error { code, .. } => Some(code),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------
// The dwell itself

#[tokio::test(start_paused = true)]
async fn a_seat_that_cannot_act_in_combat_waits_its_dwell_and_each_pass_is_a_batch_of_its_own() {
    let table = Table::start(combat(2), config(Dwell::fixed(600)), 2).await;
    table.attack().await;
    table.run_to(5_000).await;

    for n in 0..2 {
        // The attack, then a batch for each of the four windows of the two steps (the last one runs on through the
        // damage and the quiet turn of seat 1 to the next attack of seat 0).
        let times = table.batch_times(n);
        let gaps: Vec<u64> = times.windows(2).map(|pair| pair[1] - pair[0]).collect();
        assert_eq!(times[0], 0, "the attack is answered at once");
        assert_eq!(
            gaps,
            [600, 600, 600, 600],
            "each of the four windows waits a dwell of its own, the second of a step too: {times:?}"
        );
        assert_eq!(
            table.places(n),
            [
                (2, Step::DeclareAttackers),
                (2, Step::DeclareAttackers),
                (2, Step::DeclareBlockers),
                (2, Step::DeclareBlockers),
                (4, Step::DeclareAttackers)
            ],
            "seat {n}: the steps in between pass at once, inside the batch of the pass before them"
        );
        let mut next = 0;
        for (_, batch) in table.tapes[n].batches() {
            assert_eq!(batch.from_seq, next, "the batches follow one another");
            next += u64::try_from(batch.events.len()).unwrap();
        }
        assert_eq!(table.log().await.len(), usize::try_from(next).unwrap());
    }
    // Seat 0 was asked to attack, at the start and when its next turn came; nobody was asked to pass.
    assert_eq!(table.decision_times(0).len(), 2);
    assert_eq!(table.decision_times(1), Vec::<u64>::new());
}

#[tokio::test(start_paused = true)]
async fn a_spell_on_the_stack_makes_every_seat_that_cannot_answer_it_wait() {
    let table = Table::start(with_a_play_and_a_land(), config(Dwell::fixed(600)), 2).await;
    table.cast(0).await;
    table.run_to(5_000).await;

    let times = table.batch_times(1);
    // The opening, the spell paid for, seat 0's pass after a dwell (its own main phase, with its spell on the
    // stack), seat 1's pass after a dwell of its own; then the spell resolves and everything else passes at once, up
    // to the main phase of seat 1, where it can play the land it drew.
    assert_eq!(times, [0, 0, 600, 1_200]);
    assert_eq!(
        table.places(1),
        [
            (1, Step::Main1),
            (1, Step::Main1),
            (1, Step::Main1),
            (2, Step::Main1)
        ]
    );
    assert_eq!(table.decision_times(1), [times[3]]);
    // Seat 0's own main phases with an empty stack, before and after the spell, cost no time.
    assert_eq!(table.decision_times(0), [0, 0]);
}

#[tokio::test(start_paused = true)]
async fn a_seats_own_main_phase_with_an_empty_stack_is_passed_for_at_once_whatever_the_dwell() {
    let setup = quiet(2).library(1, forest_first()).setup().unwrap();
    let table = Table::start(setup, config(Dwell::fixed(600)), 2).await;
    table.run_to(5_000).await;

    // Nobody waits: the opening, then every pass of turn 1 up to the main phase of seat 1 on turn 2, where it can
    // play its land.
    assert_eq!(table.batch_times(0), [0, 0]);
    assert_eq!(table.places(0), [(1, Step::Main1), (2, Step::Main1)]);
    assert_eq!(table.decision_times(1), [0]);
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
    let table = Table::start(combat(2), config(Dwell::fixed(600)), 2).await;
    table
        .send(
            0,
            ClientMessage::Yield {
                until: YieldUntil::EndOfTurn,
            },
        )
        .await;
    table.attack().await;
    table.run_to(3_000).await;

    // Seat 0 is passed for at once in both declaring steps, though it holds nothing there either way; seat 1 waits a
    // dwell in each.
    assert_eq!(table.batch_times(1), [0, 600, 1_200]);
    assert_eq!(
        table.places(1),
        [
            (2, Step::DeclareAttackers),
            (2, Step::DeclareBlockers),
            (4, Step::DeclareAttackers)
        ]
    );
    assert_eq!(
        table.decision_times(0),
        [0, 1_200],
        "asked to attack again when its next turn comes"
    );
}

#[tokio::test(start_paused = true)]
async fn a_window_that_would_run_a_response_clock_is_passed_after_its_dwell_and_not_at_the_clock() {
    // Seat 1 is not the active player, so a window it was sent would run a 10 s clock; this one it is not sent.
    let table = Table::start(
        with_a_play_and_a_land(),
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
    table.run_to(12_000).await;

    let times = table.batch_times(1);
    assert!(
        times.last().is_some_and(|last| *last <= 1_200),
        "seat 1 was passed for after two dwells, not at the clock: {times:?}"
    );
    assert_eq!(table.places(1).last(), Some(&(2, Step::Main1)));
}

// ---------------------------------------------------------------------------------------------------------------
// Tables of idle seats

#[rstest]
#[case::four_seats(4)]
#[case::six_seats(6)]
#[tokio::test(start_paused = true)]
async fn every_idle_seat_of_a_table_waits_a_dwell_of_its_own(#[case] seats: usize) {
    let table = Table::start(combat(seats), config(Dwell::fixed(900)), seats).await;
    table.attack().await;
    let attacked_at = table.now();
    table.run_to(attacked_at + 20_000).await;

    // When each pass after the attack reached the last seat: every seat of the declare attackers step passes in turn,
    // then every seat of the declare blockers step, each after a dwell of its own, none sooner for the seats that
    // passed before it, so the step costs one dwell for each seat.
    let arrivals: Vec<u64> = table
        .batch_times(seats - 1)
        .into_iter()
        .filter(|at| *at > attacked_at)
        .map(|at| at - attacked_at)
        .collect();
    let passes = 2 * u64::try_from(seats).unwrap();
    let expected: Vec<u64> = (1..=passes).map(|pass| 900 * pass).collect();
    assert_eq!(
        arrivals[..expected.len()],
        expected[..],
        "{seats} seats in two steps"
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

/// Seat 0 attacks at once and dwells in the declare attackers step from 0 to 600; at 200 `from` sends `message`.
/// Returns the table at 700.
async fn disturbed(from: usize, message: ClientMessage, seats: usize) -> Table {
    let table = Table::start(combat(seats), config(Dwell::fixed(600)), seats).await;
    table.attack().await;
    table.run_to(200).await;
    assert_eq!(table.batch_times(0), [0], "still dwelling at 200");
    table.send(from, message).await;
    table.run_to(700).await;
    table
}

#[rstest]
#[case::a_stop_at_this_window(own(Step::DeclareAttackers))]
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
        [0, 200],
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
    // The pass came at 200, with everything the yield passes for; seat 1 then waits a dwell of its own.
    assert_eq!(table.batch_times(1), [0, 200]);
    assert_eq!(table.places(1)[1], (2, Step::DeclareAttackers));
    assert_eq!(table.decision_times(0), [0]);
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
#[case::a_stop_of_another_seat(1, own(Step::DeclareAttackers))]
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
        table.batch_times(1)[..2],
        [0, 600],
        "the pass comes when the dwell, drawn at the start, is over"
    );
    assert!(
        table.tapes[0]
            .decisions()
            .iter()
            .all(|(at, _, _)| ms(*at) == 0 || ms(*at) >= 600),
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
    assert_eq!(table.decision_times(0), [0], "only the attack came");
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
    assert_eq!(errors(&table, 0), [ErrorCode::StaleDecision]);
}

#[tokio::test(start_paused = true)]
async fn the_concession_of_a_third_seat_leaves_the_dwell_of_the_first_alone() {
    let table = disturbed(2, ClientMessage::Concede, 3).await;
    assert_eq!(
        table.batch_times(1)[..3],
        [0, 200, 600],
        "the concession is a batch of its own; seat 0 is passed for when its dwell is over"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// A pass that cannot be saved

#[tokio::test(start_paused = true)]
async fn a_pass_that_cannot_be_saved_leaves_the_game_as_it_was_and_is_tried_again() {
    let table = Table::start(combat(2), config(Dwell::fixed(600)), 2).await;
    table.attack().await;
    table.run_to(100).await;
    let before = table.log().await;
    table.store.fail(true);
    table.run_to(700).await;

    assert_eq!(table.log().await, before, "nothing was written");
    assert_eq!(table.batch_times(1), [0], "and nobody was told of a pass");
    assert_eq!(errors(&table, 0), [ErrorCode::Internal]);
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
    common::assert_same_game(
        &store.inner().load(game_id(1)).await.unwrap(),
        &straight.events,
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
    let setup = combat(2);
    let whole = Table::start(setup.clone(), config(Dwell::fixed(600)), 2).await;
    whole.attack().await;
    whole.run_to(700).await;
    let uninterrupted = whole.log().await;
    assert_eq!(whole.batch_times(0), [0, 600]);

    let first = Table::start(setup.clone(), config(Dwell::fixed(600)), 2).await;
    first.attack().await;
    first.run_to(300).await;
    let log_at_the_break = first.log().await;
    assert_eq!(first.batch_times(0), [0]);
    let store = first.store.clone();
    // Everything of the first run goes: the game ends with its last handle, and its dwell with it.
    drop(first);
    tokio::task::yield_now().await;

    let resumed_at = Instant::now();
    let handle = resume_game_with(store.clone(), game_id(1), setup, config(Dwell::fixed(600)))
        .await
        .unwrap();
    let mut tapes = Vec::new();
    for n in 0..2 {
        let inbox = handle.join(seat(n)).await.unwrap();
        tapes.push(Tape::listen(inbox, resumed_at));
    }
    // The wait left over from before is not remembered: a full dwell from the resume.
    tokio::time::sleep_until(resumed_at + Duration::from_millis(500)).await;
    handle.status().await.unwrap();
    tokio::task::yield_now().await;
    assert_eq!(store.load(game_id(1)).await.unwrap(), log_at_the_break);
    assert_eq!(tapes[1].batches().len(), 0);
    assert_eq!(
        tapes[0].decisions().len(),
        0,
        "the window of seat 0 is not handed over by the sync that comes with the join"
    );

    tokio::time::sleep_until(resumed_at + Duration::from_millis(650)).await;
    handle.status().await.unwrap();
    tokio::task::yield_now().await;
    let times: Vec<u64> = tapes[1].batches().iter().map(|(at, _)| ms(*at)).collect();
    assert_eq!(times, [600]);
    assert_eq!(store.load(game_id(1)).await.unwrap(), uninterrupted);
    assert_eq!(kind(&tapes[1].heard()[0].1), "Welcome");
}
