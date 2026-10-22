//! Goal: whole bot games with decks of Doom Blade, Swords to Plowshares, Counterspell and Wrath of God, creatures that watch deaths, die or leave the battlefield with a trigger and creatures that cannot be destroyed finish and keep every invariant (zones, life, the journal, no trigger waiting while a player has priority, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use them: permanents are destroyed, exiled and countered, a Wrath destroys several at once, spells fizzle, the triggers of the removed permanents go on the stack; an engine rebuilt from the log at each decision asks what the original asks, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::{ResolutionOutcome, ZoneCause};
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{RandomBot, play_game, removal_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`. These are short, since removal ends a game quickly.
fn games() -> u64 {
    fuzz_games(20)
}

#[derive(Default)]
struct Seen {
    destroyed: u64,
    exiled: u64,
    countered: u64,
    fizzled: u64,
    mass: u64,
    triggers: u64,
    life_gained: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        // The moves of one step come before its `StepDone`, so a step that destroys several is a mass destruction.
        let mut in_step = 0;
        for event in events {
            match event {
                Event::ZoneChanged {
                    cause: ZoneCause::Destroyed | ZoneCause::Buried,
                    ..
                } => {
                    self.destroyed += 1;
                    in_step += 1;
                }
                Event::ZoneChanged {
                    cause: ZoneCause::Exiled,
                    ..
                } => self.exiled += 1,
                Event::StepDone { .. } => {
                    self.mass += u64::from(in_step > 1);
                    in_step = 0;
                }
                Event::ResolutionEnded { outcome, .. } => match outcome {
                    ResolutionOutcome::Countered => self.countered += 1,
                    ResolutionOutcome::Fizzled => self.fizzled += 1,
                    _ => {}
                },
                Event::AbilityTriggered(_) => self.triggers += 1,
                Event::LifeGained { .. } => self.life_gained += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_removal_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = removal_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.destroyed > 0, "nothing was destroyed in {campaign}");
    assert!(seen.exiled > 0, "nothing was exiled in {campaign}");
    assert!(seen.countered > 0, "nothing was countered in {campaign}");
    assert!(seen.mass > 0, "no Wrath destroyed several in {campaign}");
    assert!(seen.fizzled > 0, "no spell fizzled in {campaign}");
    assert!(
        seen.triggers > 0,
        "no trigger went on the stack in {campaign}"
    );
    assert!(seen.life_gained > 0, "no life was gained in {campaign}");
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_through_removal(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let mut busy = 0;
    for seed in 0..8 {
        let setup = removal_fuzz_setup(players, seed).unwrap();
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
            ) && !matches!(&decision.kind, DecisionKind::ChooseTargets(o) if o.triggered);
            if !mid_cast {
                assert_eq!(
                    resumed.decision(),
                    engine.decision(),
                    "seed {seed} step {step}"
                );
            }
            busy += u64::from(!engine.state().stack().is_empty());
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    assert!(busy > 0, "no decision was met with a spell on the stack");
    Ok(())
}

#[test]
fn a_seeded_removal_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(removal_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(removal_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
