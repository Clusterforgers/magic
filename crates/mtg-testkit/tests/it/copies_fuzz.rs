//! Goal: whole bot games with decks of Clone, Phantasmal Image, Mirror Image, Cackling Counterpart, Rootborn Defenses, Growing Ranks and Followed Footsteps finish and keep every invariant (zones, life, mana, the journal, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use copies: permanents enter as copies, tokens are made as copies of permanents, and a token is made; an engine rebuilt from the log at each decision asks the same question, the choice of what to copy included, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::decision::DecisionKind;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{RandomBot, copy_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(10)
}

#[derive(Default)]
struct Seen {
    entered_as_copies: u64,
    token_copies: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::BecameCopy(_) => self.entered_as_copies += 1,
                Event::TokenCreated(token) if token.copy_of.is_some() => self.token_copies += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_copy_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = copy_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(
        seen.entered_as_copies > 0,
        "nothing entered as a copy in {campaign}"
    );
    assert!(
        seen.token_copies > 0,
        "no token was made as a copy in {campaign}"
    );
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_including_what_to_copy(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), EngineError> {
    let mut questions = 0;
    for seed in 0..8 {
        let setup = copy_fuzz_setup(players, seed).unwrap();
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
            // A cast not yet paid for and the choice of what to copy, which the replacement step holds, live outside
            // the state until they are answered; every other question is asked again by a rebuilt engine.
            let resolving = engine.state().resolving().is_some();
            let outside = engine.replacing()
                || !resolving
                    && matches!(
                        &decision.kind,
                        DecisionKind::PayMana(_)
                            | DecisionKind::ChooseTargets(_)
                            | DecisionKind::ChooseMode(_)
                    );
            if !outside {
                assert_eq!(
                    resumed.decision(),
                    engine.decision(),
                    "seed {seed} step {step}"
                );
            }
            if matches!(&decision.kind, DecisionKind::ChooseCards(options)
                if options.text.starts_with("Choose a permanent to copy"))
            {
                assert!(engine.replacing());
                questions += 1;
            }
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    assert!(
        questions > 0,
        "nobody was asked what to copy at {players} seats"
    );
    Ok(())
}

#[rstest]
fn a_seeded_copy_game_is_the_same_game_every_time(#[values(2, 3, 4)] players: usize) {
    let play = || {
        let setup = copy_fuzz_setup(players, 5).unwrap();
        play_game(setup, 5).unwrap_or_else(|failure| panic!("{failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
}
