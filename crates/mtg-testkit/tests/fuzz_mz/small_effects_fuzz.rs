//! Goal: whole bot games with decks of tapping and untapping spells, mills, spells that pick by a condition, an artifact that wins the game and creatures with flash finish and keep every invariant (zones, life, the journal, no trigger waiting while a player has priority, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use them: permanents are tapped and untapped by effects, cards are milled, a condition takes both of its branches, creatures with flash are cast out of turn, and a win ends a game with every opponent lost; an engine rebuilt from the log at each decision asks what the original asks, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::ZoneCause;
use mtg_engine::player::LossReason;
use mtg_engine::{Engine, EngineError, Event, replay};
use mtg_testkit::{RandomBot, play_game, small_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(20)
}

#[derive(Default)]
struct Seen {
    tapped_by_effects: u64,
    untapped_by_effects: u64,
    milled: u64,
    gambit_otherwise: u64,
    flashed: u64,
    wins: u64,
    ended: u64,
}

impl Seen {
    /// Reads one finished game: the log says what happened and the replay of it names the cards.
    fn add(&mut self, setup: &mtg_engine::GameSetup, events: &[Event]) {
        let state = replay(setup, events).unwrap();
        let name = |id| state.object(id).map(|o| o.card().name.clone());
        let mut resolving = false;
        let mut active = None;
        let mut main = false;
        for event in events {
            match event {
                Event::TurnBegan { active: who, .. } => active = Some(*who),
                Event::StepBegan { step } => main = step.is_main(),
                // Inside a resolution only effects tap and untap: costs are paid when a spell is cast, and the untap
                // step is turn-based.
                Event::ResolutionBegan { .. } => resolving = true,
                Event::ResolutionEnded { .. } => resolving = false,
                Event::PermanentTapped { .. } => {
                    self.tapped_by_effects += u64::from(resolving);
                }
                Event::PermanentUntapped { .. } => {
                    self.untapped_by_effects += u64::from(resolving);
                }
                Event::ZoneChanged {
                    cause: ZoneCause::Milled,
                    ..
                } => self.milled += 1,
                // Only Gambit gains life in these decks, and only when its condition does not hold.
                Event::LifeGained { .. } => self.gambit_otherwise += 1,
                Event::SpellCast { player, object, .. } => {
                    let flash = name(*object).is_some_and(|n| n == "Small Ambusher");
                    self.flashed += u64::from(flash && (Some(*player) != active || !main));
                }
                Event::PlayerLost {
                    reason: LossReason::Format(_),
                    ..
                } => self.wins += 1,
                Event::GameEnded { .. } => self.ended += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_small_effect_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = small_fuzz_setup(players, seed);
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&setup, &outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.tapped_by_effects > 0, "no effect tapped in {campaign}");
    assert!(
        seen.untapped_by_effects > 0,
        "no effect untapped in {campaign}"
    );
    assert!(seen.milled > 0, "nothing was milled in {campaign}");
    assert!(
        seen.gambit_otherwise > 0,
        "no condition took its other branch in {campaign}"
    );
    assert!(
        seen.flashed > 0,
        "no flash spell was cast out of turn in {campaign}"
    );
    assert!(seen.wins > 0, "no game was won by an effect in {campaign}");
    assert!(seen.ended > 0, "no game ended in {campaign}");
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_through_small_effects(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let mut busy = 0;
    for seed in 0..8 {
        let setup = small_fuzz_setup(players, seed);
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
fn a_seeded_small_effect_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(small_fuzz_setup(3, seed), seed).unwrap();
        let second = play_game(small_fuzz_setup(3, seed), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
