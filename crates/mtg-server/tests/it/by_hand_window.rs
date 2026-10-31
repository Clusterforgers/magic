//! Goal: a seat whose spell or ability with clauses left to the table has just resolved is asked at its next priority window, in its own main phases, its end step and another seat's turn, with 2 to 4 seats, whatever its stops or Full Control mode, until it passes or its normal response clock passes for it; the same card without such a clause is passed for at once, no other seat is asked or kept waiting, and only the held seat is sent the decision.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::time::Duration;

use common::{Client, FlakyStore, game_id, passive, windowed};
use mtg_cards::CardDefinition;
use mtg_engine::decision::{PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Decision, DecisionKind, GameSetup};
use mtg_protocol::{ClientMessage, DecisionId, FullControlMode, ServerMessage, StopSide};
use mtg_server::{Dwell, GameConfig, GameHandle, Pace, spawn_game_with};
use mtg_testkit::Scenario;
use mtg_testkit::cards::{by_hand, free_creature, giant, healer, instant};
use mtg_testkit::scenario::ready;
use rstest::rstest;
use std::sync::Arc;

/// The seat that casts or activates the card.
const ME: usize = 1;

/// A response clock of 10 s and an active-player clock of 30 s, with the dwell off.
const CLOCKS: Pace = Pace {
    auto_pass: true,
    response: Some(Duration::from_secs(10)),
    blocks: Some(Duration::from_secs(45)),
    think: Some(Duration::from_secs(60)),
    active: Some(Duration::from_secs(30)),
    extension: None,
    grace: Duration::ZERO,
    dwell: Dwell::OFF,
};

/// Where the card is used and what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Use {
    /// A creature spell in the seat's precombat main phase.
    Main1,
    /// A creature spell in its postcombat main phase.
    Main2,
    /// An instant in its own end step.
    End,
    /// A tap ability in the end step of the seat before it.
    OpponentsTurn,
}

impl Use {
    fn step(self) -> Step {
        match self {
            Use::Main1 => Step::Main1,
            Use::Main2 => Step::Main2,
            Use::End | Use::OpponentsTurn => Step::End,
        }
    }

    fn active(self) -> usize {
        if self == Use::OpponentsTurn { 0 } else { ME }
    }

    /// The card, with or without a clause left to the table.
    fn card(self, manual: bool) -> Arc<CardDefinition> {
        let card = match self {
            Use::Main1 | Use::Main2 => free_creature("Tracker", 1, 1),
            Use::End => instant("Omen", 0, &[]),
            Use::OpponentsTurn => healer("Seer", 0),
        };
        if manual {
            by_hand(
                card,
                "Whenever an enchantment you control enters, draw a card.",
            )
        } else {
            card
        }
    }

    /// `seats` seats with nothing castable in their libraries and the card in the seat's hand or on its battlefield.
    fn setup(self, seats: usize, manual: bool) -> GameSetup {
        let mut scenario = Scenario::new(seats).at(3, self.active(), self.step());
        for seat in 0..seats {
            scenario = scenario.library(seat, vec![giant(); 30]);
        }
        let card = self.card(manual);
        let scenario = if self == Use::OpponentsTurn {
            scenario.battlefield(ME, [ready(card)])
        } else {
            scenario.hand(ME, [card])
        };
        scenario.setup().unwrap()
    }
}

/// A window the game waits at: its seat, where the game stood and the decision.
struct Asked {
    seat: usize,
    place: (u32, Step),
    id: DecisionId,
    decision: Decision,
    deadline_ms: Option<u64>,
    /// Whether the stack was empty then.
    stack_empty: bool,
}

/// Hand-driven clients of one game.
struct Table {
    handle: GameHandle,
    clients: Vec<Client>,
    /// Where the latest batch each client was sent left the game.
    at: Vec<Option<(u32, Step)>>,
    /// How many objects the latest batch each client was sent left on the stack.
    stack: Vec<u32>,
    /// The seats sent a decision since the last window, other than the seat the game waits on.
    others_asked: Vec<usize>,
}

impl Table {
    async fn join(setup: GameSetup, config: GameConfig, seats: usize) -> Table {
        let handle = spawn_game_with(game_id(1), setup, FlakyStore::new(), config).unwrap();
        let mut clients = Vec::new();
        for n in 0..seats {
            clients.push(Client::join(&handle, n).await);
        }
        Table {
            handle,
            clients,
            at: vec![None; seats],
            stack: vec![0; seats],
            others_asked: Vec::new(),
        }
    }

    async fn send(&mut self, seat: usize, message: ClientMessage) {
        self.clients[seat].say(&self.handle, message).await;
        self.handle.status().await.unwrap();
    }

    async fn start(&mut self) {
        for seat in 0..self.clients.len() {
            self.send(seat, ClientMessage::Ready).await;
        }
    }

    /// The window the game waits at now, or `None` once it has ended.
    async fn window(&mut self) -> Option<Asked> {
        let waiting = self.handle.status().await.unwrap().waiting_on?.index();
        let mut found = None;
        for (n, client) in self.clients.iter_mut().enumerate() {
            for message in client.drain() {
                match message {
                    ServerMessage::Events(batch) => {
                        self.at[n] = Some((batch.table.turn.number, batch.table.turn.step));
                        self.stack[n] = batch.table.stack.count;
                    }
                    ServerMessage::Decision {
                        id,
                        decision,
                        deadline_ms,
                        ..
                    } if n == waiting => found = Some((id, decision, deadline_ms)),
                    ServerMessage::Decision { .. } => self.others_asked.push(n),
                    _ => {}
                }
            }
        }
        let (id, decision, deadline_ms) = found.expect("the seat the game waits on was sent it");
        Some(Asked {
            seat: waiting,
            place: self.at[waiting].expect("a batch came before the decision"),
            id,
            decision,
            deadline_ms,
            stack_empty: self.stack[waiting] == 0,
        })
    }

    async fn answer(&mut self, asked: &Asked, answer: Answer) {
        let decision = asked.id;
        self.send(asked.seat, ClientMessage::Answer { decision, answer })
            .await;
    }

    /// Plays the card from the seat's first window and passes over it; returns the window that follows its
    /// resolution and every window passed on the way.
    async fn use_card(&mut self, kind: Use) -> (Vec<(usize, (u32, Step))>, Option<Asked>) {
        let mut passed = Vec::new();
        let mut used = false;
        while let Some(asked) = self.window().await {
            match (&asked.decision.kind, used) {
                (DecisionKind::Priority(options), false) if asked.seat == ME => {
                    let action = if kind == Use::OpponentsTurn {
                        PriorityAction::Activate(options.abilities[0])
                    } else {
                        PriorityAction::CastSpell(options.castable[0])
                    };
                    used = true;
                    self.answer(&asked, Answer::Priority(action)).await;
                }
                (DecisionKind::PayMana(_), _) => {
                    self.answer(&asked, Answer::PayMana(PayChoice::Auto)).await;
                }
                (DecisionKind::Priority(_), true) if asked.stack_empty => {
                    return (passed, Some(asked));
                }
                _ => {
                    passed.push((asked.seat, asked.place));
                    self.answer(&asked, passive(&asked.decision)).await;
                }
            }
        }
        (passed, None)
    }
}

/// The window the game stops at once the card has resolved, with the seat in `mode` and holding a stop or not.
async fn after_resolution(
    seats: usize,
    kind: Use,
    manual: bool,
    mode: FullControlMode,
    stop: bool,
    config: GameConfig,
) -> (Table, Vec<(usize, (u32, Step))>, Option<Asked>) {
    let mut table = Table::join(kind.setup(seats, manual), config, seats).await;
    // NextWindow lets the seat use the card wherever it is; it ends with the pass over the card.
    let mode = if mode == FullControlMode::Off {
        FullControlMode::NextWindow
    } else {
        mode
    };
    table.send(ME, ClientMessage::FullControl { mode }).await;
    if stop {
        let side = if kind.active() == ME {
            StopSide::Own
        } else {
            StopSide::Opponent
        };
        let on = ClientMessage::SetStop {
            side,
            step: kind.step(),
            on: true,
            keep: true,
        };
        table.send(ME, on).await;
    }
    table.start().await;
    let (passed, next) = table.use_card(kind).await;
    (table, passed, next)
}

#[rstest]
#[tokio::test]
async fn the_seat_is_asked_after_its_by_hand_card_resolves_and_nobody_else_waits(
    #[values(2, 3, 4)] seats: usize,
    #[values(Use::Main1, Use::Main2, Use::End, Use::OpponentsTurn)] kind: Use,
    #[values(true, false)] manual: bool,
) {
    let (mut table, passed, next) =
        after_resolution(seats, kind, manual, FullControlMode::Off, false, windowed()).await;
    let held = (3, kind.step());
    assert!(
        passed
            .iter()
            .all(|(seat, place)| *seat == ME || *place != held),
        "only the card's seat was asked in its step: {passed:?}"
    );
    let at = next.as_ref().map(|asked| (asked.seat, asked.place));
    if !manual {
        // The same card without a clause left to the table: the seat is passed for at once.
        assert_ne!(at, Some((ME, held)), "{kind:?}");
        return;
    }
    assert_eq!(at, Some((ME, held)), "{kind:?}");
    let next = next.unwrap();
    assert!(
        table.others_asked.is_empty(),
        "the held decision goes to its seat alone: {:?}",
        table.others_asked
    );
    // Its pass ends the window and the game leaves the step.
    table.answer(&next, passive(&next.decision)).await;
    let after = table.window().await.expect("the game goes on");
    assert_ne!(after.place, held, "{kind:?}: {:?}", after.seat);
}

#[rstest]
#[case::no_stop(FullControlMode::Off, false)]
#[case::stop(FullControlMode::Off, true)]
#[case::full_control(FullControlMode::Held, false)]
#[case::full_control_and_stop(FullControlMode::Held, true)]
#[tokio::test]
async fn stops_and_full_control_ask_the_held_window_once(
    #[values(Use::Main2, Use::End, Use::OpponentsTurn)] kind: Use,
    #[case] mode: FullControlMode,
    #[case] stop: bool,
) {
    let (mut table, _, next) = after_resolution(3, kind, true, mode, stop, windowed()).await;
    let next = next.expect("the game goes on");
    let held = (3, kind.step());
    assert_eq!((next.seat, next.place), (ME, held), "{kind:?}");
    table.answer(&next, passive(&next.decision)).await;
    // Full Control asks the seat again later, but never again in the step it passed out of.
    let after = table.window().await.expect("the game goes on");
    assert_ne!((after.seat, after.place), (ME, held), "{kind:?}");
}

#[rstest]
#[tokio::test(start_paused = true)]
async fn the_response_clock_passes_for_a_held_seat_that_does_not_answer(
    #[values(2, 4)] seats: usize,
    #[values(Use::Main1, Use::End, Use::OpponentsTurn)] kind: Use,
) {
    let config = GameConfig {
        pace: CLOCKS,
        ..windowed()
    };
    let (mut table, _, next) =
        after_resolution(seats, kind, true, FullControlMode::Off, false, config).await;
    let next = next.expect("the game goes on");
    let held = (3, kind.step());
    assert_eq!((next.seat, next.place), (ME, held));
    // The clock the seat has at any window of that turn: the active player's, or the response clock.
    let clock = if kind.active() == ME { 30_000 } else { 10_000 };
    assert_eq!(next.deadline_ms, Some(clock));

    tokio::time::advance(Duration::from_millis(clock)).await;
    for _ in 0..50 {
        tokio::task::yield_now().await;
        table.handle.status().await.unwrap();
    }
    let after = table.window().await.expect("the game goes on");
    assert_ne!((after.seat, after.place), (ME, held));
}
