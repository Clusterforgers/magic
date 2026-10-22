//! Goal: the table cannot tell a player who can pay what an effect asks from one who cannot, by what it is sent or when. In two games with the same seed that differ in what a spell asks the opponent to pay ({1} against {2}, so the seat that holds one untapped Forest can pay in one and not in the other), the other seat is sent the same messages at the same moments, the payer is asked the same question with the same clock and its own No (or the No the clock gives it when it sits on the question) leaves the same log; a Yes from the player who cannot pay is refused to that seat alone with an error, leaves the question standing and nothing for anyone else to see, and the clock then answers No as before; the same comparison finds a player who could pay and says Yes, so the test can see a difference when there is one.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::time::Duration;

use common::{Tape, Taped, game_id, ms, passive, roomy, seat};
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

/// When each message of `heard` arrived, in milliseconds.
fn moments(heard: &[Taped]) -> Vec<u64> {
    heard.iter().map(|(at, _)| ms(*at)).collect()
}

/// What the payer is sent with the words of the question and the one flag only it is told made the same: the cost the
/// spell names is the one thing the two games differ in.
fn alike(heard: &[Taped]) -> Vec<ServerMessage> {
    told(heard)
        .into_iter()
        .map(|message| match message {
            ServerMessage::Decision {
                id,
                mut decision,
                deadline_ms,
                show_after_ms,
            } => {
                if let DecisionKind::YesNo(options) = &mut decision.kind {
                    options.only_no = false;
                    options.text = String::new();
                }
                ServerMessage::Decision {
                    id,
                    decision,
                    deadline_ms,
                    show_after_ms,
                }
            }
            other => other,
        })
        .collect()
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

#[rstest]
#[case::the_payer_answers_promptly(PROMPTLY)]
#[case::the_payer_sits_on_the_question_until_the_clock_runs_out(TOO_LONG)]
#[tokio::test(start_paused = true)]
async fn the_table_cannot_tell_a_player_who_can_pay_from_one_who_cannot(#[case] first: u64) {
    let can = play(1, declining, first, Dwell::OFF).await;
    let cannot = play(2, declining, first, Dwell::OFF).await;

    // The watcher is sent the same messages at the same moments...
    assert_eq!(told(&can.watcher), told(&cannot.watcher));
    assert_eq!(moments(&can.watcher), moments(&cannot.watcher));
    // ...though the games differ in what the spell asks and in nothing the log says...
    assert_eq!(can.log, cannot.log);
    assert_ne!(can.hash, cannot.hash);
    // ...the payer is asked in the same words about its own cost, with the same clock, at the same moment, and told
    // No is the only answer only when it cannot pay...
    assert_eq!(question(&can), ("Pay {1}?".to_owned(), false, Some(10_000)));
    assert_eq!(
        question(&cannot),
        ("Pay {2}?".to_owned(), true, Some(10_000))
    );
    assert_eq!(alike(&can.payer), alike(&cannot.payer));
    assert_eq!(moments(&can.payer), moments(&cannot.payer));
    // ...and both said no, by their own answer or the clock's.
    assert_eq!(said(&can), [false]);
    assert_eq!(said(&cannot), [false]);
    // The game went past the question and its answer: the watcher drew for the spell nobody paid.
    assert!(
        can.log.iter().any(
            |event| matches!(event, Event::CardDrawn { player, .. } if *player == seat_id(WATCHER))
        ),
        "the unpaid effect happened"
    );
    // Whatever the payer is told about a late answer, it is told alike.
    assert_eq!(errors(&can.payer), errors(&cannot.payer));
}

#[tokio::test(start_paused = true)]
async fn the_same_holds_with_the_windows_paced_by_the_default_dwell() {
    // The dwell draws its own waits, so the moments differ from game to game; what is sent does not.
    let can = play(1, declining, PROMPTLY, Dwell::DEFAULT).await;
    let cannot = play(2, declining, PROMPTLY, Dwell::DEFAULT).await;
    assert_eq!(told(&can.watcher), told(&cannot.watcher));
    assert_eq!(can.log, cannot.log);
    assert_eq!(alike(&can.payer), alike(&cannot.payer));
    assert_eq!(said(&can), [false]);
    assert_eq!(said(&cannot), [false]);
}

#[tokio::test(start_paused = true)]
async fn a_yes_from_a_player_who_cannot_pay_is_refused_to_that_seat_alone_and_the_clock_answers_no()
{
    // The payer says yes at once, is refused, and the question stands until the clock gives it a no.
    let eager_then_late = play(2, eager, PROMPTLY, Dwell::OFF).await;
    let late = play(2, declining, TOO_LONG, Dwell::OFF).await;

    assert_eq!(errors(&eager_then_late.payer), [ErrorCode::IllegalAnswer]);
    assert_eq!(errors(&eager_then_late.watcher), Vec::<ErrorCode>::new());
    // Nobody else is sent anything for it: the watcher hears what it hears when the payer says nothing at all.
    assert_eq!(told(&eager_then_late.watcher), told(&late.watcher));
    assert_eq!(moments(&eager_then_late.watcher), moments(&late.watcher));
    assert_eq!(eager_then_late.log, late.log);
    assert_eq!(said(&eager_then_late), [false]);
    // The refused answer left the question on the payer's screen with the clock running.
    let asked = eager_then_late
        .payer
        .iter()
        .filter(|(_, message)| {
            matches!(message, ServerMessage::Decision { decision, .. } if matches!(decision.kind, DecisionKind::YesNo(_)))
        })
        .count();
    assert_eq!(asked, 1, "the question was not asked again");
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
