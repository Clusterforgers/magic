//! Goal: games are reproducible from (setup, seed, answers) alone, replaying the log rebuilds the exact live state, and different seeds give different shuffles.

use std::sync::Arc;

use mtg_core::card::CardDefinition;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Normal;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, GameSetup, replay};
use mtg_testkit::{GameFailure, RandomBot, fuzz_setup, play_game};
use rstest::rstest;

fn numbered_decks(players: usize) -> Vec<Vec<Arc<CardDefinition>>> {
    (0..players)
        .map(|p| {
            (0..60)
                .map(|i| mtg_testkit::cards::free_creature(&format!("P{p} card {}", i % 15), 1, 1))
                .collect()
        })
        .collect()
}

fn shuffles(events: &[Event]) -> Vec<Vec<ObjectId>> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::LibraryShuffled { order, .. } => Some(order.clone()),
            _ => None,
        })
        .collect()
}

#[rstest]
fn the_same_seed_and_answers_give_identical_logs_and_hashes(
    #[values(2, 3, 4)] players: usize,
    #[values(0, 7, 31)] seed: u64,
) -> Result<(), GameFailure> {
    let first = play_game(fuzz_setup(players, seed), seed)?;
    let second = play_game(fuzz_setup(players, seed), seed)?;
    assert_eq!(first.events, second.events);
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.result, second.result);
    Ok(())
}

#[test]
fn different_bot_seeds_play_different_games() -> Result<(), GameFailure> {
    let setup = fuzz_setup(2, 5);
    let a = play_game(setup.clone(), 1)?;
    let b = play_game(setup, 2)?;
    assert_ne!(a.events, b.events);
    Ok(())
}

#[rstest]
fn different_game_seeds_shuffle_differently_and_equal_seeds_alike(
    #[values(2, 4)] players: usize,
) -> Result<(), EngineError> {
    let make = |seed| GameSetup::new(Arc::new(Normal), seed, numbered_decks(players));
    let orders = |seed| -> Result<Vec<Vec<ObjectId>>, EngineError> {
        Ok(shuffles(Engine::new(make(seed))?.events()))
    };
    assert_eq!(orders(1)?, orders(1)?);
    assert_ne!(orders(1)?, orders(2)?);
    let first = orders(3)?;
    assert_eq!(first.len(), players, "one shuffle per library");
    assert!(
        first.windows(2).all(|w| w[0] != w[1]),
        "each library gets its own substream, not a repeated permutation"
    );
    Ok(())
}

#[test]
fn shuffles_only_reorder_the_library() -> Result<(), EngineError> {
    let setup = GameSetup::new(Arc::new(Normal), 9, numbered_decks(3));
    let initial = mtg_engine::GameState::from_setup(&setup)?;
    let engine = Engine::new(setup)?;
    for seat in 0..3 {
        let player = mtg_core::ids::PlayerId::from_index(seat);
        let mut before: Vec<ObjectId> = initial.zone(Zone::Library(player)).to_vec();
        let mut after: Vec<ObjectId> = engine
            .state()
            .zone(Zone::Library(player))
            .iter()
            .chain(engine.state().zone(Zone::Hand(player)))
            .copied()
            .collect();
        before.sort();
        after.sort();
        assert_eq!(before, after);
    }
    Ok(())
}

#[test]
fn the_rng_counter_counts_exactly_the_random_events() -> Result<(), EngineError> {
    let engine = Engine::new(fuzz_setup(4, 12))?;
    let random_events = engine
        .events()
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::FirstPlayerChosen { .. }
                    | Event::LibraryShuffled { .. }
                    | Event::MulliganTaken { .. }
            )
        })
        .count();
    assert_eq!(
        engine.state().rng_index(),
        u64::try_from(random_events).unwrap_or(0)
    );
    assert_eq!(random_events, 5, "the first player and four shuffles");
    Ok(())
}

/// A finished game: its setup, event log and (log length, state hash) after every answer.
type Recording = (GameSetup, Vec<Event>, Vec<(usize, u64)>);

/// Plays a game and records (log length, state hash, pending decision) after every answer.
fn record_game(players: usize, seed: u64) -> Result<Recording, EngineError> {
    let setup = fuzz_setup(players, seed);
    let mut engine = Engine::new(setup.clone())?;
    let mut bots: Vec<RandomBot> = (0..players as u64)
        .map(|i| RandomBot::new(seed, i))
        .collect();
    let mut marks = vec![(engine.events().len(), engine.state_hash())];
    while let Some(decision) = engine.decision().cloned() {
        let answer = bots[decision.player.index()].choose(&decision);
        engine.apply(answer)?;
        marks.push((engine.events().len(), engine.state_hash()));
    }
    Ok((setup, engine.events().to_vec(), marks))
}

#[rstest]
fn replaying_any_prefix_of_the_log_rebuilds_the_live_state(
    #[values(2, 5)] players: usize,
    #[values(3, 17)] seed: u64,
) -> Result<(), EngineError> {
    let (setup, events, marks) = record_game(players, seed)?;
    for (len, hash) in marks.iter().step_by(37).chain(marks.last()) {
        let rebuilt = replay(&setup, &events[..*len])?;
        assert_eq!(rebuilt.state_hash(), *hash, "prefix of {len} events");
    }
    Ok(())
}

#[rstest]
fn resuming_from_a_prefix_reaches_the_same_pending_decision(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let setup = fuzz_setup(players, 8);
    let mut engine = Engine::new(setup.clone())?;
    let mut bots: Vec<RandomBot> = (0..players as u64).map(|i| RandomBot::new(8, i)).collect();
    for step in 0..400 {
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        if step % 41 == 0 {
            let resumed = Engine::resume(setup.clone(), engine.events().to_vec())?;
            assert_eq!(resumed.state_hash(), engine.state_hash());
            if matches!(decision.kind, DecisionKind::PayMana(_)) {
                // A half-finished cast lives outside game state, so a rebuilt engine asks for priority again.
                let again = resumed.decision().map(|d| (d.player, d.kind.name()));
                assert_eq!(again, Some((decision.player, "priority")));
            } else {
                assert_eq!(resumed.decision(), engine.decision());
            }
        }
        let answer = bots[decision.player.index()].choose(&decision);
        engine.apply(answer)?;
    }
    Ok(())
}

#[test]
fn replay_rejects_logs_that_do_not_follow_from_the_setup() {
    let setup = fuzz_setup(2, 1);
    let bogus = [Event::CardDrawn {
        player: mtg_core::ids::PlayerId(0),
        object: ObjectId(0),
    }];
    assert!(matches!(
        replay(&setup, &bogus),
        Err(EngineError::InvalidEvent(_))
    ));
}

#[test]
fn bots_with_the_same_seed_answer_identically() {
    let engine = Engine::new(fuzz_setup(2, 3)).map(|e| e.decision().cloned());
    let decision = engine.ok().flatten();
    let Some(decision) = decision else {
        panic!("a fresh game asks for a mulligan decision");
    };
    let answers = |seed| RandomBot::new(seed, 0).choose(&decision);
    assert_eq!(answers(5), answers(5));
}
