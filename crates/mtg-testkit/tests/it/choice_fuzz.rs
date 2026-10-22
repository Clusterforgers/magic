//! Goal: whole bot games with decks of "may" spells, choices among opponents and charms with one, two and repeated modes finish and keep every invariant (zones, life, the journal, no trigger waiting while a player has priority, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use the decisions: modes are chosen, a mode is chosen twice, "may" questions are answered both ways and opponents are chosen; an engine rebuilt from the log at each decision asks the same question, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::choose::Choice;
use mtg_engine::decision::DecisionKind;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{RandomBot, choice_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

#[derive(Default)]
struct Seen {
    modal_casts: u64,
    repeats: u64,
    said_yes: u64,
    said_no: u64,
    options: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::SpellCast { choices, .. } if !choices.modes.is_empty() => {
                    self.modal_casts += 1;
                    self.repeats +=
                        u64::from(choices.modes.windows(2).any(|pair| pair[0] == pair[1]));
                    assert!(
                        choices.modes.windows(2).all(|pair| pair[0] <= pair[1]),
                        "the modes are kept in printed order: {:?}",
                        choices.modes
                    );
                }
                Event::ChoiceMade {
                    choice: Choice::YesNo(yes),
                    ..
                } => {
                    self.said_yes += u64::from(*yes);
                    self.said_no += u64::from(!*yes);
                }
                Event::ChoiceMade {
                    choice: Choice::Option(_),
                    ..
                } => self.options += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_choice_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let outcome = play_game(choice_fuzz_setup(players, seed), seed)
            .unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(
        seen.modal_casts > 0,
        "no modal spell was cast in {campaign}"
    );
    assert!(seen.said_yes > 0, "no one chose to in {campaign}");
    assert!(seen.said_no > 0, "no one chose not to in {campaign}");
    if players > 2 {
        assert!(seen.options > 0, "no opponent was chosen in {campaign}");
    } else {
        assert_eq!(seen.options, 0, "the only opponent is not asked about");
    }
}

#[test]
fn a_mode_is_chosen_twice_in_some_games() {
    let mut seen = Seen::default();
    for seed in 0..200 {
        let outcome = play_game(choice_fuzz_setup(3, seed), seed)
            .unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
        if seen.repeats > 0 {
            return;
        }
    }
    panic!("no mode was chosen twice in 200 games");
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_about_modes_and_questions(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let mut asked = 0;
    for seed in 0..8 {
        let setup = choice_fuzz_setup(players, seed);
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
            // A half-finished cast lives outside the state, so a rebuilt engine asks for priority again.
            let mid_cast = matches!(
                &decision.kind,
                DecisionKind::PayMana(_)
                    | DecisionKind::ChooseMode(_)
                    | DecisionKind::ChooseTargets(_)
            ) && !matches!(&decision.kind, DecisionKind::ChooseTargets(o) if o.triggered);
            if matches!(
                decision.kind,
                DecisionKind::ChooseMode(_)
                    | DecisionKind::YesNo(_)
                    | DecisionKind::ChooseOption(_)
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
    assert!(asked > 0, "no mode, question or option was asked");
    Ok(())
}

#[test]
fn a_seeded_choice_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(choice_fuzz_setup(3, seed), seed).unwrap();
        let second = play_game(choice_fuzz_setup(3, seed), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
