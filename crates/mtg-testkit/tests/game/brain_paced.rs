//! Goal: a paced bot (a simulator's) keeps its level's search but turns each tree search's clock into the work it buys
//! at a live table, never searches more rollouts than its level's count, and its games replay with the seed, at every
//! searching level.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};

use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::Answer;
use mtg_engine::{Decision, Engine};
use mtg_testkit::brain::clock::MOST;
use mtg_testkit::brain::{Deep, Level, PACED_WORK, SeatBrain, Settings, deep, paced_brain};
use mtg_testkit::brain_decks::{Duel, duel_setup};
use mtg_testkit::harness::{PlayOptions, play_brains};
use rstest::rstest;

#[rstest]
#[case::normal(Settings::NORMAL, None)]
#[case::hard(Settings::HARD, Some(Deep::LIGHT))]
#[case::very_hard(Settings::VERY_HARD, Some(Deep::FULL))]
fn a_paced_search_keeps_its_level_but_does_the_work_its_clock_buys(
    #[case] settings: Settings,
    #[case] deep: Option<Deep>,
) {
    let paced = settings.paced();
    assert_eq!(paced.time, None);
    assert_eq!(paced.sims, settings.sims);
    assert_eq!(paced.combat_search, settings.combat_search);
    assert_eq!(paced.deep.is_some(), deep.is_some());
    if let (Some(paced), Some(deep)) = (paced.deep, deep) {
        let clock = deep.time.unwrap().as_secs_f64();
        assert_eq!(paced.iterations, deep.iterations);
        assert_eq!(deep.work, None);
        let work = f64::from(paced.work.unwrap());
        assert!((work - clock * PACED_WORK).abs() <= 0.5);
    }
}

/// The most rollouts one tree search played, of the brain it wraps.
struct Counted {
    inner: Box<dyn SeatBrain>,
    most: Arc<Mutex<u64>>,
}

impl SeatBrain for Counted {
    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer {
        let (searches_before, rollouts_before) = (deep::tally().0, deep::iterations());
        let answer = self.inner.choose(decision, table);
        let searched = deep::tally().0 - searches_before;
        let played = deep::iterations() - rollouts_before;
        if let Some(each) = played.checked_div(searched) {
            let mut most = self.most.lock().unwrap();
            *most = (*most).max(each);
        }
        answer
    }
}

/// The events of a duel of paced `level` bots seeded with `seed` over at most `answers` answers, and the most
/// rollouts one tree search played.
fn paced_game(level: Level, seed: u64, answers: usize) -> (Vec<mtg_engine::events::Event>, u64) {
    let setup = duel_setup(&[Duel::Dimir, Duel::Gruul], seed, HouseRules::default()).unwrap();
    let most = Arc::new(Mutex::new(0));
    let mut brains: Vec<Box<dyn SeatBrain>> = (0..2)
        .map(|seat| {
            Box::new(Counted {
                inner: paced_brain(level, seed, PlayerId::from_index(seat)),
                most: Arc::clone(&most),
            }) as Box<dyn SeatBrain>
        })
        .collect();
    let options = PlayOptions {
        max_answers: answers,
        ..PlayOptions::default()
    };
    // A game cut at the answer limit keeps its events.
    let events = match play_brains(setup, seed, options, &mut brains) {
        Ok(outcome) => outcome.events,
        Err(failure) => failure.events,
    };
    let most = *most.lock().unwrap();
    (events, most)
}

#[rstest]
#[case::hard(Level::Hard, Settings::HARD)]
#[case::very_hard(Level::VeryHard, Settings::VERY_HARD)]
fn a_paced_game_stays_within_its_count_and_replays(
    #[case] level: Level,
    #[case] settings: Settings,
) {
    // In range: a few rollouts at most three times over.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let cap = (f64::from(settings.paced().deep.unwrap().iterations) * MOST).round() as u64;
    let (first, most) = paced_game(level, 3, 300);
    assert_ne!(first.len(), 0);
    assert!(most > 0, "no tree search ran");
    assert!(
        most <= cap,
        "a tree search played {most} rollouts, over {cap}"
    );
    let (again, _) = paced_game(level, 3, 300);
    assert_eq!(first, again);
}
