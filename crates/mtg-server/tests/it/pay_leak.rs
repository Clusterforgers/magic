//! Goal: the table does not ask a seat a payment it cannot make from what is in sight, and what that shows is what the battlefield already shows. In two games with the same seed that differ in what a spell asks the opponent to pay ({1} against {2}, so the seat that holds one untapped Forest can pay in one and not in the other), the seat that cannot pay is sent no question, is refused nothing and leaves a No in the log at once, at any pace; the seat that can pay is asked in the words of the card with its response clock; the same comparison finds a player who could pay and says Yes, so the test can see a difference when there is one.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::time::Duration;

use common::{Tape, Taped, game_id, passive, roomy, seat};
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{Choice, YesNoChoice};
use mtg_engine::decision::{PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Decision, DecisionKind, Event, GameSetup};
use mtg_protocol::{ClientMessage, ErrorCode, ServerMessage};
use mtg_server::{Dwell, EventStore, GameConfig, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::cards::{forest, giant, island};
use mtg_testkit::pay_cards::toll_costing;
use mtg_testkit::{Scenario, ready};
use rstest::rstest;
use tokio::time::Instant;

/// Event batches the watcher is sent before a game is cut.
const BATCHES: usize = 10;

/// The pause before the payer answers: well inside the response clock.
const PROMPTLY: u64 = 450;

/// A pause longer than the response clock (10 s): the payer sits on the question until the table answers for it.
const TOO_LONG: u64 = 20_000;

/// The seat that casts the spell and watches (0) and the one asked to pay (1).
const WATCHER: usize = 0;
const PAYER: usize = 1;

/// Seat 0 casts "Toll" in the first main phase of turn 2 (an Island pays for it): "Unless an opponent pays {tax}, you
/// draw a card; if one does, you gain 1 life." Seat 1 has one untapped Forest, so it can pay {1} and not {2}.
fn game(tax: u32) -> GameSetup {
    Scenario::new(2)
        .seed(11)
        .library(0, vec![giant(); 30])
        .library(1, vec![giant(); 30])
        .at(2, 0, Step::Main1)
        .hand(0, [toll_costing("Toll", tax)])
        .battlefield(0, [ready(island())])
        .battlefield(1, [ready(forest())])
        .setup()
        .unwrap()
}

/// Casts what it can, paying automatically, and passes otherwise.
fn caster(decision: &Decision) -> Answer {
    match &decision.kind {
        DecisionKind::Priority(options) => Answer::Priority(match options.castable.first() {
            Some(spell) => PriorityAction::CastSpell(*spell),
            None => PriorityAction::Pass,
        }),
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        _ => passive(decision),
    }
}

/// Says no to a question and passes otherwise.
fn declining(decision: &Decision) -> Answer {
    match &decision.kind {
        DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(false)),
        _ => passive(decision),
    }
}

/// Says yes to a question, pays with whatever it has and passes otherwise.
fn eager(decision: &Decision) -> Answer {
    match &decision.kind {
        DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        _ => passive(decision),
    }
}

/// What one game showed.
struct Run {
    /// Everything the watcher was sent, up to its [`BATCHES`]th batch.
    watcher: Vec<Taped>,
    /// Everything the payer was sent, up to the moment the watcher's last batch arrived.
    payer: Vec<Taped>,
    /// The event log up to the last event the watcher was sent.
    log: Vec<Event>,
    /// The state hash at the end.
    hash: Option<u64>,
}

/// Plays a game with the response clock and the windows of the default pace and `dwell`: the watcher casts the spell,
/// the payer answers its first question with `brain` after `first` milliseconds.
async fn play(tax: u32, brain: fn(&Decision) -> Answer, first: u64, dwell: Dwell) -> Run {
    let since = Instant::now();
    let store = MemoryStore::new();
    let config = GameConfig {
        pace: Pace {
            dwell,
            ..Pace::default()
        },
        ..roomy()
    };
    let handle = spawn_game_with(game_id(1), game(tax), store.clone(), config).unwrap();
    let brains: [fn(&Decision) -> Answer; 2] = [caster, brain];
    // The payer answers its first question after `first` and every other decision promptly.
    let payer_thinks = std::iter::once(first)
        .chain([PROMPTLY; 15])
        .map(Duration::from_millis);
    let thinking = [vec![Duration::from_millis(300)], payer_thinks.collect()];
    let mut tapes = Vec::new();
    for (n, think) in thinking.into_iter().enumerate() {
        let inbox = handle.join(seat(n)).await.unwrap();
        tapes.push(Tape::play(
            inbox,
            since,
            handle.clone(),
            seat(n),
            think,
            brains[n],
        ));
    }
    for n in 0..2 {
        handle.send(seat(n), ClientMessage::Ready).await.unwrap();
    }
    while since.elapsed() < Duration::from_secs(120) && batches(&tapes[WATCHER].heard()) < BATCHES {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let status = handle.status().await.unwrap();
    let mut heard: Vec<Vec<Taped>> = tapes
        .into_iter()
        .map(|tape| {
            let heard = tape.heard();
            tape.stop();
            heard
        })
        .collect();
    let payer = heard.remove(PAYER);
    let watcher = cut(heard.remove(0));
    let seen = watcher
        .iter()
        .rev()
        .find_map(|(_, message)| match message {
            ServerMessage::Events(batch) => {
                Some(usize::try_from(batch.from_seq).unwrap() + batch.events.len())
            }
            _ => None,
        })
        .unwrap();
    let arrived = watcher.last().map_or(Duration::ZERO, |(at, _)| *at);
    Run {
        payer: payer.into_iter().filter(|(at, _)| *at <= arrived).collect(),
        watcher,
        log: store.load(game_id(1)).await.unwrap()[..seen].to_vec(),
        hash: status.state_hash,
    }
}

/// How many event batches `heard` holds.
fn batches(heard: &[Taped]) -> usize {
    heard
        .iter()
        .filter(|(_, message)| matches!(message, ServerMessage::Events(_)))
        .count()
}

/// `heard` up to and including its [`BATCHES`]th batch.
fn cut(heard: Vec<Taped>) -> Vec<Taped> {
    let mut seen = 0;
    let mut kept = Vec::new();
    for (at, message) in heard {
        seen += usize::from(matches!(message, ServerMessage::Events(_)));
        kept.push((at, message));
        if seen == BATCHES {
            break;
        }
    }
    kept
}

/// The messages of `heard` as they are on the wire.
fn told(heard: &[Taped]) -> Vec<ServerMessage> {
    heard.iter().map(|(_, message)| message.clone()).collect()
}

/// The question the payer was asked, with the time left on its clock: the first yes-or-no decision it was sent.
fn question(run: &Run) -> (String, bool, Option<u64>) {
    run.payer
        .iter()
        .find_map(|(_, message)| match message {
            ServerMessage::Decision {
                decision,
                deadline_ms,
                ..
            } => match &decision.kind {
                DecisionKind::YesNo(options) => {
                    Some((options.text.clone(), options.only_no, *deadline_ms))
                }
                _ => None,
            },
            _ => None,
        })
        .expect("the payer was asked whether to pay")
}

/// What the payer said to the question in the log, if it said anything.
fn said(run: &Run) -> Vec<bool> {
    run.log
        .iter()
        .filter_map(|event| match event {
            Event::ChoiceMade {
                player,
                choice: Choice::YesNo(yes),
            } if *player == seat_id(PAYER) => Some(*yes),
            _ => None,
        })
        .collect()
}

fn seat_id(n: usize) -> PlayerId {
    seat(n)
}

/// The errors a client was sent.
fn errors(heard: &[Taped]) -> Vec<ErrorCode> {
    heard
        .iter()
        .filter_map(|(_, message)| match message {
            ServerMessage::Error { code, .. } => Some(*code),
            _ => None,
        })
        .collect()
}

/// The yes-or-no questions the payer was sent.
fn questions(run: &Run) -> usize {
    run.payer
        .iter()
        .filter(|(_, message)| {
            matches!(message, ServerMessage::Decision { decision, .. } if matches!(decision.kind, DecisionKind::YesNo(_)))
        })
        .count()
}

#[rstest]
#[case::the_payer_answers_promptly(PROMPTLY, Dwell::OFF)]
#[case::the_payer_would_sit_on_the_question(TOO_LONG, Dwell::OFF)]
#[case::with_the_default_dwell(PROMPTLY, Dwell::DEFAULT)]
#[tokio::test(start_paused = true)]
async fn a_payment_the_seat_cannot_make_from_what_is_in_sight_is_declined_without_asking(
    #[case] first: u64,
    #[case] dwell: Dwell,
) {
    let cannot = play(2, eager, first, dwell).await;

    // The payer is never asked and cannot answer a question it was not sent: nothing is refused.
    assert_eq!(questions(&cannot), 0);
    assert_eq!(errors(&cannot.payer), Vec::<ErrorCode>::new());
    // The table said No for it and the game went on: the watcher drew for the spell nobody paid.
    assert_eq!(said(&cannot), [false]);
    assert!(
        cannot.log.iter().any(
            |event| matches!(event, Event::CardDrawn { player, .. } if *player == seat_id(WATCHER))
        ),
        "the unpaid effect happened"
    );
}

#[tokio::test(start_paused = true)]
async fn a_seat_that_can_pay_is_asked_with_its_clock_and_the_table_does_not_answer_for_it() {
    let can = play(1, declining, PROMPTLY, Dwell::OFF).await;
    let cannot = play(2, declining, PROMPTLY, Dwell::OFF).await;
    assert_eq!(questions(&can), 1);
    assert_eq!(question(&can), ("Pay {1}?".to_owned(), false, Some(10_000)));
    assert_eq!(said(&can), [false]);
    assert_ne!(can.hash, cannot.hash);
}

#[tokio::test(start_paused = true)]
async fn a_player_who_can_pay_and_says_yes_shows_it_so_the_comparison_can_see_a_difference() {
    let pays = play(1, eager, PROMPTLY, Dwell::OFF).await;
    let cannot = play(2, eager, PROMPTLY, Dwell::OFF).await;
    assert_ne!(told(&pays.watcher), told(&cannot.watcher));
    assert_eq!(
        said(&pays),
        [true, true],
        "the yes and the yes of the payment"
    );
    assert_eq!(said(&cannot), [false]);
    assert_eq!(errors(&pays.payer), Vec::<ErrorCode>::new());
}
