//! Goal: in bot games where seats concede at random times, the active player more often, every seat and a spectator see the same turn view at every step: the flag is set exactly while the seat that `active` names has lost (judged from the event log, a second opinion on the engine's state), never inside a mulligan or a finished game, never goes out inside a turn, and is clear in the turn after it; a batch carries the view of the state it ends in, and a game resumed from its log at the first flagged step and at the end shows the same views; invariants of the engine hold throughout, at tables of two to four seats and decks of plain play, combat keywords, spells and triggers.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_core::ids::PlayerId;
use mtg_engine::rng::GameRng;
use mtg_engine::state::Stage;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, Event, GameSetup};
use mtg_protocol::{StateView, TableView, project_events};
use mtg_testkit::{
    InvariantChecker, RandomBot, fuzz_setup, keyword_fuzz_setup, spell_fuzz_setup,
    trigger_fuzz_setup,
};
use rstest::rstest;

/// Games per table size and deck family; `MTG_FUZZ_GAMES` overrides it.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(12)
}

/// Upper bound on answers in one game.
const MAX_ANSWERS: usize = 200_000;

/// Out of every 1000 decisions, how many are answered by a concession instead.
const CONCEDE_PER_MILLE: u64 = 8;

/// What a campaign saw.
#[derive(Default)]
struct Totals {
    /// Views taken of a turn without its active player.
    flagged: u64,
    /// Views taken of a turn with its active player.
    unflagged: u64,
    /// The steps in which a view said the turn had no active player.
    steps: BTreeSet<Step>,
}

/// The seat that concedes next, if one does: a roll against the rate, then the active player for most rolls.
fn conceder(engine: &Engine, rng: &mut GameRng) -> Option<PlayerId> {
    if rng.below(1000) >= CONCEDE_PER_MILLE {
        return None;
    }
    let state = engine.state();
    let alive: Vec<PlayerId> = state.alive_players().collect();
    if alive.len() < 2 {
        return None;
    }
    match state.active_player() {
        Some(active) if rng.below(5) < 3 => Some(active),
        _ => Some(alive[rng.index(alive.len())]),
    }
}

/// Whether the log says `seat` has lost.
fn has_lost(engine: &Engine, seat: PlayerId) -> bool {
    engine
        .events()
        .iter()
        .any(|e| matches!(e, Event::PlayerLost { player, .. } if *player == seat))
}

/// Every viewer's table: each seat, then a spectator.
fn tables(engine: &Engine, seats: usize) -> Vec<TableView> {
    (0..=seats)
        .map(|viewer| TableView::of(engine.state(), PlayerId::from_index(viewer)))
        .collect()
}

/// Checks the turn view of every viewer against the log and records what it said.
fn check_turn_views(engine: &Engine, seats: usize, seed: u64, totals: &mut Totals) -> bool {
    let state = engine.state();
    let turn = state.turn();
    let expected = state.stage() == Stage::Turns && has_lost(engine, turn.active());
    for (viewer, view) in tables(engine, seats).iter().enumerate() {
        let context = format!("seed {seed}, viewer {viewer}, turn {}", turn.number());
        assert_eq!(view.turn.active, turn.active(), "{context}");
        assert_eq!(view.turn.number, turn.number(), "{context}");
        assert_eq!(view.turn.active_left, expected, "{context}");
    }
    if expected {
        totals.flagged += 1;
        totals.steps.insert(turn.step());
    } else {
        totals.unflagged += 1;
    }
    expected
}

fn assert_resumed_views(setup: &GameSetup, engine: &Engine, seats: usize, seed: u64) {
    let resumed = Engine::resume(setup.clone(), engine.events().to_vec()).expect("log resumes");
    for viewer in (0..=seats).map(PlayerId::from_index) {
        assert!(
            StateView::for_player(resumed.state(), viewer)
                == StateView::for_player(engine.state(), viewer),
            "seed {seed}, viewer {viewer:?}: a resumed game shows another view"
        );
    }
}

fn play(setup: &GameSetup, seed: u64, totals: &mut Totals) {
    let seats = setup.players.len();
    let mut engine = Engine::new(setup.clone()).expect("fuzz setups are valid");
    let mut checker = InvariantChecker::new(setup)
        .expect("fresh checker")
        .with_full_check_every(25);
    let mut bots: Vec<RandomBot> = (0u64..)
        .take(seats)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    let mut schedule = GameRng::substream(seed, 7_100);
    let viewers: Vec<PlayerId> = (0..=seats).map(PlayerId::from_index).collect();
    let mut resumed_once = false;
    let mut previous: Option<(u32, bool)> = None;
    for _ in 0..MAX_ANSWERS {
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        let before = engine.state().clone();
        let from_seq = engine.events().len() as u64;
        let events = if let Some(seat) = conceder(&engine, &mut schedule) {
            engine
                .concede(seat)
                .expect("a seat in the game may concede")
        } else {
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer).expect("bot answers are legal")
        };
        checker
            .observe_engine(&engine)
            .unwrap_or_else(|v| panic!("seed {seed}: {v}"));
        let flagged = check_turn_views(&engine, seats, seed, totals);
        let number = engine.state().turn().number();
        if let Some((turn, was_flagged)) = previous {
            assert!(
                turn != number
                    || flagged
                    || !was_flagged
                    || engine.state().stage() == Stage::Finished,
                "seed {seed}: the flag went out inside turn {number} of a game that goes on"
            );
        }
        previous = Some((number, flagged));
        for (batch, viewer) in project_events(&before, &events, &viewers, from_seq)
            .expect("events project")
            .iter()
            .zip(&viewers)
        {
            assert!(
                *batch.table == TableView::of(engine.state(), *viewer),
                "seed {seed}: a batch carries another turn view than the state it ends in"
            );
        }
        if flagged && !resumed_once {
            resumed_once = true;
            assert_resumed_views(setup, &engine, seats, seed);
        }
    }
    assert!(
        engine.decision().is_none(),
        "seed {seed}: the game did not finish"
    );
    for view in tables(&engine, seats) {
        assert!(
            !view.turn.active_left,
            "seed {seed}: a finished game has no turn that lacks its player"
        );
    }
    assert_resumed_views(setup, &engine, seats, seed);
}

type Family = fn(usize, u64) -> GameSetup;

#[rstest]
#[case::plain_play(2, fuzz_setup)]
#[case::plain_play(3, fuzz_setup)]
#[case::plain_play(4, fuzz_setup)]
#[case::combat_keywords(4, keyword_fuzz_setup)]
#[case::spells(3, spell_fuzz_setup)]
#[case::triggers(4, trigger_fuzz_setup)]
fn the_turn_view_follows_the_departures_of_random_games(
    #[case] seats: usize,
    #[case] family: Family,
) {
    let mut totals = Totals::default();
    for seed in 0..games() {
        play(&family(seats, seed), seed, &mut totals);
    }
    assert!(totals.unflagged > 0, "no view of an ordinary turn");
    if seats > 2 {
        assert!(
            totals.flagged > 0 && totals.steps.len() >= 3,
            "too few turns without an active player: {} views in steps {:?}",
            totals.flagged,
            totals.steps
        );
    }
}
