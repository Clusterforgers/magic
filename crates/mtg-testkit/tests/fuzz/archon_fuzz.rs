//! Goal: whole bot games with decks of Archon of Cruelty, edicts, discard spells, planeswalkers and creatures finish and keep every invariant (zones, life, the journal, no trigger waiting while a player has priority, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use the choice of cards: players are asked to sacrifice and discard by an ability or spell another seat controls, permanents are sacrificed and cards discarded; an engine rebuilt from the log at each decision asks the same question, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use std::collections::BTreeMap;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::Choice;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::ZoneCause;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{RandomBot, archon_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`. These are short, since an Archon decides a game quickly.
fn games() -> u64 {
    fuzz_games(60)
}

#[derive(Default)]
struct Seen {
    choices: u64,
    asked_of_another_seat: u64,
    sacrificed: u64,
    discarded: u64,
    abilities: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        let mut controllers: BTreeMap<ObjectId, PlayerId> = BTreeMap::new();
        let mut resolving = None;
        for event in events {
            match event {
                Event::SpellCast { player, object, .. } => {
                    controllers.insert(*object, *player);
                }
                Event::AbilityPutOnStack(put) => {
                    controllers.insert(put.object, put.controller);
                    self.abilities += u64::from(put.cause.is_some());
                }
                Event::ResolutionBegan { object } => resolving = controllers.get(object).copied(),
                Event::ChoiceMade {
                    player,
                    choice: Choice::Cards(_),
                } => {
                    self.choices += 1;
                    self.asked_of_another_seat +=
                        u64::from(resolving.is_some_and(|c| c != *player));
                }
                Event::ZoneChanged {
                    cause: ZoneCause::Sacrificed,
                    ..
                } => self.sacrificed += 1,
                Event::CardDiscarded { .. } => self.discarded += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_archon_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = archon_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(
        seen.abilities > 0,
        "no Archon trigger went on the stack in {campaign}"
    );
    assert!(seen.choices > 0, "no cards were chosen in {campaign}");
    assert!(
        seen.asked_of_another_seat > 0,
        "no player was asked by the ability or spell of another in {campaign}"
    );
    assert!(seen.sacrificed > 0, "nothing was sacrificed in {campaign}");
    assert!(seen.discarded > 0, "nothing was discarded in {campaign}");
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_about_cards(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let mut asked = 0;
    for seed in 0..8 {
        let setup = archon_fuzz_setup(players, seed).unwrap();
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
            if matches!(decision.kind, DecisionKind::ChooseCards(_)) {
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
    assert!(asked > 0, "no choice of cards was asked");
    Ok(())
}

#[test]
fn a_seeded_archon_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(archon_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(archon_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
