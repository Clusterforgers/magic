//! Goal: whole bot games with decks of Opt, Brainstorm and spells that scry, look at the top of libraries, put cards on the bottom of them, shuffle and reveal finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use the library effects: players scry and choose which cards go down and in what order, cards are put back on top and on the bottom, libraries are shuffled by spells, hands are revealed; an engine rebuilt from the log at each decision asks the same question, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::choose::Choice;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{RandomBot, library_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`. These are short, since the decks have few threats.
fn games() -> u64 {
    fuzz_games(40)
}

#[derive(Default)]
struct Seen {
    scries: u64,
    scried_to_the_top: u64,
    scried_to_the_bottom: u64,
    orders: u64,
    bottomed: u64,
    put_on_top: u64,
    shuffled_by_spells: u64,
    looked: u64,
    revealed: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        let opened = events
            .iter()
            .position(|event| matches!(event, Event::MulliganPhaseEnded))
            .unwrap_or(events.len());
        for (at, event) in events.iter().enumerate() {
            match event {
                Event::Scried(scried) => {
                    self.scries += 1;
                    self.scried_to_the_top += u64::from(!scried.top.is_empty());
                    self.scried_to_the_bottom += u64::from(!scried.bottom.is_empty());
                }
                Event::ChoiceMade {
                    choice: Choice::Order(_),
                    ..
                } => self.orders += 1,
                Event::LibraryBottomed { .. } => self.bottomed += 1,
                Event::ZoneChanged {
                    to: Zone::Library(_),
                    ..
                } => self.put_on_top += 1,
                Event::LibraryShuffled { .. } if at > opened => self.shuffled_by_spells += 1,
                Event::LibraryLooked(_) => self.looked += 1,
                Event::Revealed(_) => self.revealed += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_library_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = library_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.scries > 0, "nobody scried in {campaign}");
    assert!(
        seen.scried_to_the_top > 0 && seen.scried_to_the_bottom > 0,
        "scries kept cards on top and put cards on the bottom in {campaign}"
    );
    assert!(
        seen.orders > 0,
        "no cards were put in an order in {campaign}"
    );
    assert!(
        seen.bottomed > 0,
        "no card went to the bottom in {campaign}"
    );
    assert!(
        seen.put_on_top > 0,
        "no card was put into a library in {campaign}"
    );
    assert!(
        seen.shuffled_by_spells > 0,
        "no spell shuffled a library in {campaign}"
    );
    assert!(seen.looked > 0, "no library was looked at in {campaign}");
    assert!(seen.revealed > 0, "no hand was revealed in {campaign}");
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_about_libraries(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let mut asked = 0;
    for seed in 0..8 {
        let setup = library_fuzz_setup(players, seed).unwrap();
        let mut engine = Engine::new(setup.clone())?;
        let mut bots: Vec<RandomBot> = (0..players as u64)
            .map(|seat| RandomBot::new(seed, seat))
            .collect();
        for step in 0..1500 {
            let Some(decision) = engine.decision().cloned() else {
                break;
            };
            let resumed = Engine::resume(setup.clone(), engine.events().to_vec())?;
            assert_eq!(
                resumed.state_hash(),
                engine.state_hash(),
                "seed {seed} step {step}"
            );
            let mid_cast = matches!(
                &decision.kind,
                DecisionKind::PayMana(_) | DecisionKind::ChooseTargets(_)
            );
            if matches!(
                decision.kind,
                DecisionKind::ChooseCards(_) | DecisionKind::OrderCards(_)
            ) {
                asked += 1;
            }
            if !mid_cast {
                assert_eq!(
                    resumed.decision(),
                    engine.decision(),
                    "seed {seed} step {step}"
                );
            }
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    assert!(asked > 0, "no choice of cards or order was asked");
    Ok(())
}

#[test]
fn a_seeded_library_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(library_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(library_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
