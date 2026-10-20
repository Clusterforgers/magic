//! Goal: whole bot games with decks of Rampant Growth and Cultivate finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really search: libraries are searched with and without a find, lands enter the battlefield tapped, Cultivate reveals what it finds, libraries are shuffled by the spells; an engine rebuilt from the log at each decision asks the same question, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{RandomBot, play_game, ramp_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

#[derive(Default)]
struct Seen {
    found_something: u64,
    found_nothing: u64,
    revealed: u64,
    entered_tapped: u64,
    shuffled: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::CardsFound(found) if found.cards.is_empty() => self.found_nothing += 1,
                Event::CardsFound(_) => self.found_something += 1,
                Event::Revealed(_) => self.revealed += 1,
                Event::Entered { tapped: true, .. } => self.entered_tapped += 1,
                Event::LibraryShuffled { .. } => self.shuffled += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_ramp_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = ramp_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(
        seen.found_something > 0,
        "nobody found a card in {campaign}"
    );
    assert!(seen.found_nothing > 0, "nobody found nothing in {campaign}");
    assert!(seen.revealed > 0, "nothing was revealed in {campaign}");
    assert!(
        seen.entered_tapped > 0,
        "no land entered tapped in {campaign}"
    );
    assert!(seen.shuffled > 0, "no library was shuffled in {campaign}");
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_about_a_search(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let mut asked = 0;
    for seed in 0..8 {
        let setup = ramp_fuzz_setup(players, seed).unwrap();
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
            asked += usize::from(matches!(decision.kind, DecisionKind::ChooseCards(_)));
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
    assert!(asked > 0, "no search was asked");
    Ok(())
}

#[test]
fn a_seeded_ramp_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(ramp_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(ramp_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
