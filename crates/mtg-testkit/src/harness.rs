//! Runs bot games to completion while checking every invariant, failing with the seed and log.

use std::fmt;

use mtg_engine::state::Stage;
use mtg_engine::{Engine, EngineError, Event, GameResult, GameSetup};
use thiserror::Error;

use crate::bot::RandomBot;
use crate::invariants::{InvariantChecker, Violation};

/// Limits and switches for [`play_game_with`].
#[derive(Debug, Clone, Copy)]
pub struct PlayOptions {
    /// Give up after this many answers.
    pub max_answers: usize,
    /// Check invariants after every event and answer.
    pub check_invariants: bool,
}

impl Default for PlayOptions {
    fn default() -> Self {
        PlayOptions {
            max_answers: 200_000,
            check_invariants: true,
        }
    }
}

/// A finished game.
#[derive(Debug, Clone)]
pub struct GameOutcome {
    /// How it ended.
    pub result: GameResult,
    /// Answers given.
    pub answers: usize,
    /// The turn it ended on.
    pub turns: u32,
    /// Hash of the final state.
    pub final_hash: u64,
    /// The complete event log.
    pub events: Vec<Event>,
}

/// What went wrong in a bot game.
#[derive(Debug, Clone, Error)]
pub enum FailureKind {
    /// An invariant broke.
    #[error("invariant violated: {0}")]
    Invariant(#[from] Violation),
    /// The engine rejected an answer the bot built from the legal options, or failed internally.
    #[error("engine error: {0}")]
    Engine(#[from] EngineError),
    /// The game did not finish within the answer limit.
    #[error("no result after {0} answers")]
    StepLimit(usize),
    /// The game ended in an inconsistent way.
    #[error("bad ending: {0}")]
    BadEnding(String),
}

/// A failed bot game, carrying everything needed to reproduce it.
#[derive(Clone)]
pub struct GameFailure {
    /// The bot seed (the game seed is in the setup and is usually the same value).
    pub seed: u64,
    /// What failed.
    pub kind: FailureKind,
    /// The events up to the failure.
    pub events: Vec<Event>,
}

impl fmt::Debug for GameFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for GameFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "game failed with seed {}: {}", self.seed, self.kind)?;
        writeln!(f, "event log ({} events):", self.events.len())?;
        for (i, event) in self.events.iter().enumerate() {
            writeln!(f, "  {i:>5}: {event:?}")?;
        }
        Ok(())
    }
}

/// Plays `setup` to the end with one seeded [`RandomBot`] per seat and default options.
pub fn play_game(setup: GameSetup, seed: u64) -> Result<GameOutcome, GameFailure> {
    play_game_with(setup, seed, PlayOptions::default())
}

/// Like [`play_game`] with explicit limits.
pub fn play_game_with(
    setup: GameSetup,
    seed: u64,
    options: PlayOptions,
) -> Result<GameOutcome, GameFailure> {
    let fail = |kind: FailureKind, events: &[Event]| GameFailure {
        seed,
        kind,
        events: events.to_vec(),
    };
    let mut checker = if options.check_invariants {
        Some(InvariantChecker::new(&setup).map_err(|v| fail(v.into(), &[]))?)
    } else {
        None
    };
    let mut bots: Vec<RandomBot> = (0u64..)
        .take(setup.players.len())
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    let mut engine = Engine::new(setup).map_err(|e| fail(e.into(), &[]))?;
    observe(&mut checker, &engine).map_err(|v| fail(v.into(), engine.events()))?;

    let mut answers = 0;
    while let Some(decision) = engine.decision().cloned() {
        if answers >= options.max_answers {
            return Err(fail(FailureKind::StepLimit(answers), engine.events()));
        }
        let Some(bot) = bots.get_mut(decision.player.index()) else {
            let message = format!("decision for unknown {:?}", decision.player);
            return Err(fail(FailureKind::BadEnding(message), engine.events()));
        };
        let answer = bot.choose(&decision);
        engine
            .apply(answer)
            .map_err(|e| fail(e.into(), engine.events()))?;
        answers += 1;
        observe(&mut checker, &engine).map_err(|v| fail(v.into(), engine.events()))?;
    }
    finish(&engine, checker.as_ref(), answers).map_err(|kind| fail(kind, engine.events()))
}

fn observe(checker: &mut Option<InvariantChecker>, engine: &Engine) -> Result<(), Violation> {
    match checker {
        Some(checker) => checker.observe(engine.state(), engine.events()),
        None => Ok(()),
    }
}

/// Terminal checks: one `GameEnded`, a consistent winner, and replay equality for the whole log.
fn finish(
    engine: &Engine,
    checker: Option<&InvariantChecker>,
    answers: usize,
) -> Result<GameOutcome, FailureKind> {
    let state = engine.state();
    let bad = |message: &str| FailureKind::BadEnding(message.to_owned());
    let ended: Vec<&GameResult> = engine
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::GameEnded { result } => Some(result),
            _ => None,
        })
        .collect();
    let [result] = ended[..] else {
        return Err(bad("expected exactly one GameEnded event"));
    };
    if state.stage() != Stage::Finished || state.result() != Some(*result) {
        return Err(bad("state does not record the result"));
    }
    let survivors: Vec<_> = state.alive_players().collect();
    match (result, survivors.as_slice()) {
        (GameResult::Winner(winner), [only]) if winner == only => {}
        (GameResult::Draw, []) => {}
        _ => return Err(bad("result does not match the players still in the game")),
    }
    if let Some(checker) = checker {
        checker.check_full_replay(state, engine.events())?;
    }
    Ok(GameOutcome {
        result: *result,
        answers,
        turns: state.turn().number(),
        final_hash: state.state_hash(),
        events: engine.events().to_vec(),
    })
}
