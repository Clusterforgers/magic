//! Goal: timed bots answer every decision within the decision cap, below the server's 5 s think deadline, at every
//! searching level and on decks with counters, removal and flash; a stop set around a decision (its time, or a flag
//! the caller raises) ends every search soon after, on duel and Commander pod boards alike.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::Answer;
use mtg_engine::{Decision, Engine};
use mtg_testkit::brain::clock::{DECISION_CAP, Stop, stopping};
use mtg_testkit::brain::{Level, SeatBrain, brain};
use mtg_testkit::brain_decks::{Duel, duel_setup};
use mtg_testkit::harness::{PlayOptions, play_brains};
use rstest::rstest;

/// The server's think deadline: no decision may take longer.
const DEADLINE: Duration = Duration::from_secs(5);

/// A brain that keeps the longest answer of its table.
struct Stopwatch {
    inner: Box<dyn SeatBrain>,
    longest: Arc<Mutex<Duration>>,
}

impl SeatBrain for Stopwatch {
    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer {
        let started = Instant::now();
        let answer = self.inner.choose(decision, table);
        let took = started.elapsed();
        let mut longest = self.longest.lock().unwrap();
        *longest = (*longest).max(took);
        answer
    }
}

/// The longest answer of timed `level` bots at both seats of a game of `decks` seeded with `seed`, over at most
/// `answers` answers.
fn longest_answer(level: Level, decks: [Duel; 2], seed: u64, answers: usize) -> Duration {
    let setup = duel_setup(&decks, seed, HouseRules::default()).unwrap();
    let longest = Arc::new(Mutex::new(Duration::ZERO));
    let mut brains: Vec<Box<dyn SeatBrain>> = (0..2)
        .map(|seat| {
            Box::new(Stopwatch {
                inner: brain(level, seed, PlayerId::from_index(seat), true),
                longest: Arc::clone(&longest),
            }) as Box<dyn SeatBrain>
        })
        .collect();
    let options = PlayOptions {
        max_answers: answers,
        ..PlayOptions::default()
    };
    // A game cut at the answer limit is fine: only the answers' times count.
    let _ = play_brains(setup, seed, options, &mut brains);
    *longest.lock().unwrap()
}

#[rstest]
#[case::hard(Level::Hard, [Duel::Dimir, Duel::Izzet], 600)]
#[case::very_hard(Level::VeryHard, [Duel::Azorius, Duel::Gruul], 160)]
fn every_decision_ends_within_the_cap(
    #[case] level: Level,
    #[case] decks: [Duel; 2],
    #[case] answers: usize,
) {
    let longest = longest_answer(level, decks, 11, answers);
    assert!(DECISION_CAP < DEADLINE);
    assert!(
        longest <= DECISION_CAP,
        "the longest answer took {longest:?}"
    );
}

/// How far past its stop an answer may run: the step the search was in, and the answer built after it.
const OVERSHOOT: Duration = Duration::from_millis(250);

/// A brain whose every answer runs under a stop `budget` after it is asked, or under a raised flag when `budget` is
/// `None`, keeping the longest time past it.
struct Stopped {
    inner: Box<dyn SeatBrain>,
    budget: Option<Duration>,
    over: Arc<Mutex<Duration>>,
}

impl SeatBrain for Stopped {
    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer {
        let started = Instant::now();
        let stop = match self.budget {
            Some(budget) => Stop::at(started + budget),
            None => Stop::default().or_cancel(Arc::new(AtomicBool::new(true))),
        };
        let answer = stopping(stop, || self.inner.choose(decision, table));
        let past = started
            .elapsed()
            .saturating_sub(self.budget.unwrap_or_default());
        let mut over = self.over.lock().unwrap();
        *over = (*over).max(past);
        answer
    }
}

#[rstest]
#[case::hard_duel(Level::Hard, Some(60), false)]
#[case::very_hard_duel(Level::VeryHard, Some(60), false)]
#[case::very_hard_pod(Level::VeryHard, Some(60), true)]
#[case::very_hard_cancelled_pod(Level::VeryHard, None, true)]
fn a_stop_ends_every_search_soon_after(
    #[case] level: Level,
    #[case] budget_ms: Option<u64>,
    #[case] pod: bool,
) {
    let seed = 5;
    let (setup, seats) = if pod {
        let setup = mtg_testkit::commander_cards::commander_fuzz_setup(4, seed).unwrap();
        (setup, 4)
    } else {
        let decks = [Duel::Azorius, Duel::Gruul];
        (duel_setup(&decks, seed, HouseRules::default()).unwrap(), 2)
    };
    let over = Arc::new(Mutex::new(Duration::ZERO));
    let mut brains: Vec<Box<dyn SeatBrain>> = (0..seats)
        .map(|seat| {
            Box::new(Stopped {
                inner: brain(level, seed, PlayerId::from_index(seat), true),
                budget: budget_ms.map(Duration::from_millis),
                over: Arc::clone(&over),
            }) as Box<dyn SeatBrain>
        })
        .collect();
    let options = PlayOptions {
        max_answers: 400,
        ..PlayOptions::default()
    };
    // A game cut at the answer limit is fine: only how far each answer ran past its stop counts.
    let _ = play_brains(setup, seed, options, &mut brains);
    let over = *over.lock().unwrap();
    assert!(over <= OVERSHOOT, "an answer ran {over:?} past its stop");
}
