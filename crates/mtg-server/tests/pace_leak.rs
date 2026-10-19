//! Goal: the table cannot tell a seat with a play from one without by what it is sent or when. In two games with the same seed that differ by one card in a hidden hand, a creature the seat can cast against one it cannot afford, the other seats are sent the same kinds of message in the same order and the same batches, each arriving 300 to 900 ms after the one before it on a paused clock, and nothing that depends on the hand (a decision, a clock, a yield, a stop) reaches them; with the dwell switched off the same comparison finds the difference.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::{Tape, Taped, game_id, kind, ms, roomy, seat};
use mtg_engine::turn::Step;
use mtg_engine::{DecisionKind, Event, GameSetup};
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::{Dwell, EventStore, GameConfig, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::cards::{bear, forest, giant};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;
use tokio::time::Instant;

/// How long the bot that holds the card takes to pass at each of its windows: a person's pause, inside the dwell range.
const THINKING_MS: [u64; 8] = [430, 780, 340, 620, 880, 510, 700, 390];

/// Batches the first of the other seats is sent before a game is cut.
const BATCHES: usize = 12;

/// The card in the holder's hand that the two games do not share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Card {
    /// A 2/2 for {1}{G}: the holder has two Forests, so it can cast it at every main phase.
    Castable,
    /// A 4/4 for {3}{G}{G}: the holder cannot pay for it, so it has nothing to do.
    Unaffordable,
}

/// A table whose libraries hold nothing anybody can use. The `holder` has two Forests and `card` in its hand; nobody
/// else has anything, so at the main phase of the holder the seat can act or cannot by that card alone.
fn setup(seats: usize, holder: usize, card: Card) -> GameSetup {
    let card = match card {
        Card::Castable => bear(),
        Card::Unaffordable => giant(),
    };
    let mut scenario = Scenario::new(seats).seed(11);
    for n in 0..seats {
        scenario = scenario.library(n, vec![giant(); 30]);
    }
    scenario
        .battlefield(holder, [ready(forest()), ready(forest())])
        .hand(holder, [card])
        .setup()
        .unwrap()
}

/// What one game showed.
struct Run {
    /// Per seat that does not hold the card: everything it was sent, with arrival times, up to the last batch counted.
    others: Vec<Vec<Taped>>,
    /// Everything the holder was sent.
    holder: Vec<Taped>,
    /// The event log at the end.
    log: Vec<Event>,
    /// The state hash at the end.
    hash: Option<u64>,
}

/// Plays a game with the server's default pace but `dwell`: the holder is a bot that passes after a pause, the others
/// only listen. It ends once the first other seat has been sent [`BATCHES`] batches, or after a minute.
async fn play(seats: usize, holder: usize, card: Card, dwell: Dwell) -> Run {
    let since = Instant::now();
    let store = MemoryStore::new();
    let config = GameConfig {
        pace: Pace {
            dwell,
            ..Pace::default()
        },
        ..roomy()
    };
    let setup = setup(seats, holder, card);
    let handle = spawn_game_with(game_id(1), setup, store.clone(), config).unwrap();
    let thinking: Vec<Duration> = THINKING_MS.into_iter().map(Duration::from_millis).collect();
    let mut tapes = Vec::new();
    for n in 0..seats {
        let inbox = handle.join(seat(n)).await.unwrap();
        tapes.push(if n == holder {
            Tape::play(inbox, since, handle.clone(), seat(n), thinking.clone())
        } else {
            Tape::listen(inbox, since)
        });
    }
    for n in 0..seats {
        handle.send(seat(n), ClientMessage::Ready).await.unwrap();
    }
    let watcher = (holder + 1) % seats;
    while since.elapsed() < Duration::from_secs(60) && tapes[watcher].batches().len() < BATCHES {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let status = handle.status().await.unwrap();
    let mut others = Vec::new();
    let mut held = Vec::new();
    for (n, tape) in tapes.into_iter().enumerate() {
        let heard = tape.heard();
        tape.stop();
        if n == holder {
            held = heard;
        } else {
            others.push(cut(heard));
        }
    }
    Run {
        others,
        holder: held,
        log: store.load(game_id(1)).await.unwrap(),
        hash: status.state_hash,
    }
}

/// `heard` up to and including its [`BATCHES`]th batch.
fn cut(heard: Vec<Taped>) -> Vec<Taped> {
    let mut batches = 0;
    let mut kept = Vec::new();
    for (at, message) in heard {
        let is_batch = matches!(message, ServerMessage::Events(_));
        kept.push((at, message));
        batches += usize::from(is_batch);
        if batches == BATCHES {
            break;
        }
    }
    kept
}

/// The messages of `heard` as they are on the wire, without when they arrived.
fn told(heard: &[Taped]) -> Vec<ServerMessage> {
    heard.iter().map(|(_, message)| message.clone()).collect()
}

/// The kinds of message in `heard`, in order.
fn kinds(heard: &[Taped]) -> Vec<String> {
    heard.iter().map(|(_, message)| kind(message)).collect()
}

/// Where each batch in `heard` starts in the log, how many events it holds and where the game stood after it.
fn boundaries(heard: &[Taped]) -> Vec<(u64, usize, (u32, Step))> {
    heard
        .iter()
        .filter_map(|(_, message)| match message {
            ServerMessage::Events(batch) => Some((
                batch.from_seq,
                batch.events.len(),
                (batch.table.turn.number, batch.table.turn.step),
            )),
            _ => None,
        })
        .collect()
}

/// Milliseconds between each batch in `heard` and the one before it.
fn gaps(heard: &[Taped]) -> Vec<u64> {
    let arrivals: Vec<u64> = heard
        .iter()
        .filter(|(_, message)| matches!(message, ServerMessage::Events(_)))
        .map(|(at, _)| ms(*at))
        .collect();
    arrivals.windows(2).map(|pair| pair[1] - pair[0]).collect()
}

/// The wait the server's default dwell draws from, in milliseconds: the range every batch has to arrive in.
fn dwell_range() -> std::ops::RangeInclusive<u64> {
    let (shortest, longest) = Dwell::DEFAULT.range();
    ms(shortest)..=ms(longest)
}

/// What an observer may be sent while it waits: a `Welcome`, the lobby before the game and the game's events. Anything
/// else (a decision with its clock, `Yielding`, `Stops`, `FullControl`, a snapshot, an error) can depend on a hand.
const MAY_BE_SENT: [&str; 3] = ["Welcome", "Lobby", "Events"];

/// Everything that differs between what the seats that do not hold the card were sent in the two games, and everything
/// they were sent that could depend on the hand; empty when the table cannot tell the games apart.
fn differences(with_a_play: &Run, without: &Run) -> Vec<String> {
    let mut found = Vec::new();
    for (n, (a, b)) in with_a_play.others.iter().zip(&without.others).enumerate() {
        if kinds(a) != kinds(b) {
            found.push(format!(
                "seat {n}: the messages it is sent differ: {:?} against {:?}",
                kinds(a),
                kinds(b)
            ));
        }
        if boundaries(a) != boundaries(b) {
            found.push(format!(
                "seat {n}: the batches differ: {:?} against {:?}",
                boundaries(a),
                boundaries(b)
            ));
        }
        if told(a) != told(b) {
            found.push(format!("seat {n}: it is told different things"));
        }
        for (label, heard) in [("with a play", a), ("without one", b)] {
            for gap in gaps(heard) {
                if !dwell_range().contains(&gap) {
                    found.push(format!(
                        "seat {n}, {label}: a batch came {gap} ms after the one before"
                    ));
                }
            }
            for sent in kinds(heard) {
                if !MAY_BE_SENT.contains(&sent.as_str()) {
                    found.push(format!("seat {n}, {label}: it was sent a {sent}"));
                }
            }
        }
    }
    found
}

/// The windows the holder was asked about, as whether it could act at each.
fn asked(run: &Run) -> Vec<bool> {
    run.holder
        .iter()
        .filter_map(|(_, message)| match message {
            ServerMessage::Decision { decision, .. } => match &decision.kind {
                DecisionKind::Priority(options) => Some(options.can_act),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[rstest]
#[case::two_seats_the_first_to_move_holds_the_card(2, 0)]
#[case::two_seats_the_second_holds_it(2, 1)]
#[case::three_seats_the_middle_one_holds_it(3, 1)]
#[tokio::test(start_paused = true)]
async fn the_table_cannot_tell_a_seat_with_a_play_from_one_without(
    #[case] seats: usize,
    #[case] holder: usize,
) {
    assert!(
        THINKING_MS
            .iter()
            .all(|pause| dwell_range().contains(pause)),
        "the holder thinks as long as a dwell lasts, or the pause alone would give it away"
    );
    let with_a_play = play(seats, holder, Card::Castable, Dwell::DEFAULT).await;
    let without = play(seats, holder, Card::Unaffordable, Dwell::DEFAULT).await;

    // The games differ in one hidden card and in nothing the log says...
    assert_eq!(with_a_play.log, without.log);
    assert_ne!(with_a_play.hash, without.hash);
    // ...so that the holder is asked at its windows in one game and at none in the other.
    let asked_with = asked(&with_a_play);
    assert!(
        asked_with.len() >= 3 && asked_with.iter().all(|can_act| *can_act),
        "the holder with a play is asked, able to act: {asked_with:?}"
    );
    assert_eq!(
        asked(&without),
        Vec::<bool>::new(),
        "the holder without one is asked nothing"
    );
    for run in [&with_a_play, &without] {
        assert_eq!(run.others.len(), seats - 1);
        assert!(
            run.others
                .iter()
                .all(|heard| boundaries(heard).len() == BATCHES),
            "every game was followed for {BATCHES} batches"
        );
    }

    // What the other seats can see of that is nothing.
    assert_eq!(differences(&with_a_play, &without), Vec::<String>::new());
}

#[rstest]
#[case::two_seats_the_first_to_move_holds_the_card(2, 0)]
#[case::two_seats_the_second_holds_it(2, 1)]
#[case::three_seats_the_middle_one_holds_it(3, 1)]
#[tokio::test(start_paused = true)]
async fn with_the_dwell_switched_off_the_same_comparison_finds_the_tell(
    #[case] seats: usize,
    #[case] holder: usize,
) {
    // MTG_DWELL_MS=0: an idle window is passed for at once, in the batch of the step before it.
    let with_a_play = play(seats, holder, Card::Castable, Dwell::OFF).await;
    let without = play(seats, holder, Card::Unaffordable, Dwell::OFF).await;

    let found = differences(&with_a_play, &without);
    assert!(
        found
            .iter()
            .any(|difference| difference.contains("batches differ")),
        "the pace tells the games apart without a dwell: {found:?}"
    );
    assert!(
        found
            .iter()
            .any(|difference| difference.contains("ms after")),
        "and so does the time each batch takes to arrive: {found:?}"
    );
}
