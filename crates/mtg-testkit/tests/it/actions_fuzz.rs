//! Goal: whole bot games at two, three and four seats with the keyword-action cards (Consider, Curate, Notion Rain, Sinister Sabotage, Doom Whisperer) and a spell that makes its controller the monarch finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state) for many seeds, and across the seeds players surveil, keep cards on top and put cards into the graveyard, the monarch designation changes hands, an engine rebuilt from the log at each decision asks the same question, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{RandomBot, actions_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(30)
}

#[derive(Default)]
struct Seen {
    surveils: u64,
    kept_on_top: u64,
    sent_to_the_graveyard: u64,
    monarchs: u64,
    taken_from_another: u64,
    proliferates: u64,
    explores: u64,
    connives: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        let mut monarch = None;
        for event in events {
            match event {
                Event::Surveilled(surveilled) => {
                    self.surveils += 1;
                    self.kept_on_top += u64::from(!surveilled.top.is_empty());
                    self.sent_to_the_graveyard += u64::from(!surveilled.graveyard.is_empty());
                }
                Event::Proliferated { .. } => self.proliferates += 1,
                Event::Explored { .. } => self.explores += 1,
                Event::Connived { .. } => self.connives += 1,
                Event::MonarchChanged { player } => {
                    self.monarchs += 1;
                    self.taken_from_another += u64::from(monarch.is_some_and(|was| was != *player));
                    monarch = Some(*player);
                }
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_action_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let outcome = play_game(actions_fuzz_setup(players, seed), seed)
            .unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.surveils > 0, "nobody surveilled in {campaign}");
    assert!(seen.proliferates > 0, "nobody proliferated in {campaign}");
    assert!(seen.explores > 0, "no creature explored in {campaign}");
    assert!(seen.connives > 0, "no creature connived in {campaign}");
    assert!(
        seen.kept_on_top > 0 && seen.sent_to_the_graveyard > 0,
        "surveils kept cards on top and put cards into the graveyard in {campaign}"
    );
    assert!(seen.monarchs > 0, "nobody became the monarch in {campaign}");
    assert!(
        seen.taken_from_another > 0,
        "the monarch designation changed hands in {campaign}"
    );
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_about_actions(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    for seed in 0..6 {
        let setup = actions_fuzz_setup(players, seed);
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
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    Ok(())
}

#[test]
fn a_seeded_action_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(actions_fuzz_setup(3, seed), seed).unwrap();
        let second = play_game(actions_fuzz_setup(3, seed), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
