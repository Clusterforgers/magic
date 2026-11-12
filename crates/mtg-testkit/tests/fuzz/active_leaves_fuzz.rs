//! Goal: bot games in which seats concede at random times, the active player more often than the others, keep every invariant (zones, the journal, the log replaying to the live state, the next turn going to the next player in turn order, no decision asked of a seat that left or of a missing active player) at tables of two, three and four seats with decks of plain play, combat keywords, spells and triggers, and the campaign really runs turns without an active player through several steps.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::sync::Mutex;

use crate::common::seeds::{for_each_seed, fuzz_games};
use mtg_core::ids::PlayerId;
use mtg_engine::player::LossReason;
use mtg_engine::rng::GameRng;
use mtg_engine::state::Stage;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, Event, GameSetup};
use mtg_testkit::{
    InvariantChecker, RandomBot, fuzz_setup, keyword_fuzz_setup, spell_fuzz_setup,
    trigger_fuzz_setup,
};
use rstest::rstest;

/// Games per table size and deck family; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

/// Upper bound on answers in one game.
const MAX_ANSWERS: usize = 200_000;

/// Out of every 1000 decisions, how many are answered by a concession instead.
const CONCEDE_PER_MILLE: u64 = 5;

/// What a campaign saw.
#[derive(Default)]
struct Totals {
    /// Concessions made by the active player.
    active_left: u64,
    /// Concessions made by another seat.
    other_left: u64,
    /// Decisions asked in turns that had no active player.
    decisions_without_active: u64,
    /// Turns that ended after their active player had left.
    turns_run_out: u64,
    /// The steps the active player left in.
    steps: BTreeSet<Step>,
}

/// The seat that concedes next, if one does: a roll against the rate, then the active player for most rolls.
fn conceder(engine: &Engine, rng: &mut GameRng) -> Option<PlayerId> {
    if rng.below(1000) >= CONCEDE_PER_MILLE {
        return None;
    }
    let state = engine.state();
    let alive: Vec<PlayerId> = state.alive_players().collect();
    if alive.is_empty() {
        return None;
    }
    match state.active_player() {
        Some(active) if rng.below(5) < 3 => Some(active),
        _ => Some(alive[rng.index(alive.len())]),
    }
}

fn play(setup: &GameSetup, seed: u64, totals: &Mutex<Totals>) {
    let players = setup.players.len();
    let mut engine = Engine::new(setup.clone()).expect("fuzz setups are valid");
    let mut checker = InvariantChecker::new(setup)
        .expect("fresh checker")
        .with_full_check_every(25);
    checker
        .observe_engine(&engine)
        .expect("invariants at the start");
    let mut bots: Vec<RandomBot> = (0u64..)
        .take(players)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    let mut schedule = GameRng::substream(seed, 7_000);
    let mut mine = Totals::default();
    for _ in 0..MAX_ANSWERS {
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        let state = engine.state();
        let (turn_active, step, running) = (
            state.turn().active(),
            state.turn().step(),
            state.stage() == Stage::Turns,
        );
        if running && state.active_player().is_none() {
            mine.decisions_without_active += 1;
        }
        let events = if let Some(seat) = conceder(&engine, &mut schedule) {
            if running && seat == turn_active {
                mine.active_left += 1;
                mine.steps.insert(step);
            } else {
                mine.other_left += 1;
            }
            engine
                .concede(seat)
                .expect("a seat in the game may concede")
        } else {
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer).expect("bot answers are legal")
        };
        // A turn that began while the previous active player was out ran out without one.
        let gone = !engine.state().is_alive(turn_active);
        mine.turns_run_out +=
            u64::from(gone && events.iter().any(|e| matches!(e, Event::TurnBegan { .. })));
        checker
            .observe_engine(&engine)
            .unwrap_or_else(|v| panic!("seed {seed}: {v}"));
    }
    assert!(
        engine.decision().is_none(),
        "seed {seed}: the game did not finish"
    );
    checker
        .full_check(engine.state())
        .unwrap_or_else(|v| panic!("seed {seed}: {v}"));
    checker
        .check_full_replay(engine.state(), engine.events())
        .unwrap_or_else(|v| panic!("seed {seed}: {v}"));
    let resumed = Engine::resume(setup.clone(), engine.events().to_vec()).expect("log resumes");
    assert_eq!(resumed.state_hash(), engine.state_hash(), "seed {seed}");
    let conceded = engine
        .events()
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::PlayerLost {
                    reason: LossReason::Conceded,
                    ..
                }
            )
        })
        .count();
    assert!(
        conceded < players,
        "seed {seed}: the last seat cannot concede"
    );
    let mut all = totals.lock().unwrap();
    all.active_left += mine.active_left;
    all.other_left += mine.other_left;
    all.decisions_without_active += mine.decisions_without_active;
    all.turns_run_out += mine.turns_run_out;
    all.steps.extend(mine.steps);
}

type Family = fn(usize, u64) -> GameSetup;

#[rstest]
#[case::plain_play(2, fuzz_setup)]
#[case::plain_play(3, fuzz_setup)]
#[case::plain_play(4, fuzz_setup)]
#[case::combat_keywords(3, keyword_fuzz_setup)]
#[case::combat_keywords(4, keyword_fuzz_setup)]
#[case::spells(3, spell_fuzz_setup)]
#[case::spells(4, spell_fuzz_setup)]
#[case::triggers(3, trigger_fuzz_setup)]
#[case::triggers(4, trigger_fuzz_setup)]
fn random_concessions_keep_every_invariant_and_turns_run_out_without_an_active_player(
    #[case] players: usize,
    #[case] family: Family,
) {
    let totals = Mutex::new(Totals::default());
    for_each_seed(0..games(), |seed| {
        play(&family(players, seed), seed, &totals);
    });
    let totals = totals.into_inner().unwrap();
    assert!(totals.active_left > 0, "the active player never left");
    assert!(totals.other_left > 0, "no other seat ever left");
    if players > 2 {
        assert!(
            totals.decisions_without_active > 0 && totals.turns_run_out > 0,
            "no turn ran without its active player: {} decisions, {} turns",
            totals.decisions_without_active,
            totals.turns_run_out
        );
        assert!(
            totals.steps.len() >= 5,
            "the active player left in too few steps: {:?}",
            totals.steps
        );
    }
}
