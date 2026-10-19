//! Runs bot games to completion while checking every invariant, failing with the seed and log.

use std::fmt;

use mtg_engine::decision::{Answer, PriorityAction};
use mtg_engine::state::Stage;
use mtg_engine::{Engine, EngineError, Event, GameResult, GameSetup};
use thiserror::Error;

use crate::bot::RandomBot;
use crate::invariants::{InvariantChecker, Violation};
use crate::windows::{Mode, SeatWindows, Windows};

/// Whole-state invariants run every this many answers; per-event checks run on every event.
const FULL_CHECK_EVERY: usize = 25;

/// Limits and switches for [`play_game_with`].
// Independent switches that tests set by name, not states of one machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct PlayOptions {
    /// Give up after this many answers.
    pub max_answers: usize,
    /// Check invariants after every event and answer.
    pub check_invariants: bool,
    /// Let the bots pay with part of a payment for the engine to finish (see [`RandomBot::with_partial_payments`]).
    pub partial_payments: bool,
    /// Let the bots choose how to pay hybrid and Phyrexian symbols, life included (see [`RandomBot::with_symbol_choices`]).
    pub symbol_choices: bool,
    /// Play under the default windows of a server with auto-pass on ([`Windows`]): a holder the windows pass for is
    /// not asked of its bot. Off, every seat is in Full Control `Held` and is asked at every window, as on a bare engine.
    pub auto_pass: bool,
    /// Let each bot choose its stops and Full Control mode before the game ([`RandomBot::windows`]), as a seat that
    /// sets them in the lobby does; they replace what `auto_pass` gives that seat.
    pub bot_windows: bool,
}

impl Default for PlayOptions {
    fn default() -> Self {
        PlayOptions {
            max_answers: 200_000,
            check_invariants: true,
            partial_payments: false,
            symbol_choices: false,
            auto_pass: false,
            bot_windows: false,
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
    /// Passes the windows made for seats instead of asking their bots (0 unless [`PlayOptions::auto_pass`] or
    /// [`PlayOptions::bot_windows`] lets a seat out of Full Control); they count in `answers` too.
    pub auto_passes: usize,
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
    setup
        .validate()
        .map_err(|e| fail(EngineError::from(e).into(), &[]))?;
    let mut checker = if options.check_invariants {
        Some(
            InvariantChecker::new(&setup)
                .map_err(|v| fail(v.into(), &[]))?
                .with_full_check_every(FULL_CHECK_EVERY),
        )
    } else {
        None
    };
    let mut bots: Vec<RandomBot> = (0u64..)
        .take(setup.players.len())
        .map(|seat| {
            let mut bot = RandomBot::new(seed, seat);
            if options.partial_payments {
                bot = bot.with_partial_payments();
            }
            if options.symbol_choices {
                bot = bot.with_symbol_choices();
            }
            bot
        })
        .collect();
    let mut engine = Engine::new(setup).map_err(|e| fail(e.into(), &[]))?;
    observe(&mut checker, &engine).map_err(|v| fail(v.into(), engine.events()))?;

    let start = if options.auto_pass {
        Mode::Off
    } else {
        Mode::Held
    };
    let seats = bots
        .iter_mut()
        .map(|bot| {
            if options.bot_windows {
                bot.windows()
            } else {
                SeatWindows::in_mode(start)
            }
        })
        .collect();
    let mut windows = Windows::new(seats, engine.state());
    windows.observe(engine.events());

    let pass = Answer::Priority(PriorityAction::Pass);
    let mut answers = 0;
    let mut auto_passes = 0;
    while let Some(decision) = engine.decision().cloned() {
        if answers >= options.max_answers {
            return Err(fail(FailureKind::StepLimit(answers), engine.events()));
        }
        let Some(bot) = bots.get_mut(decision.player.index()) else {
            let message = format!("decision for unknown {:?}", decision.player);
            return Err(fail(FailureKind::BadEnding(message), engine.events()));
        };
        let table_passes = windows.passes(engine.state(), &decision);
        let answer = if table_passes {
            auto_passes += 1;
            pass.clone()
        } else {
            bot.choose(&decision)
        };
        let seat_passes = !table_passes && answer == pass;
        let events = engine
            .apply(answer)
            .map_err(|e| fail(e.into(), engine.events()))?;
        windows.observe(&events);
        if seat_passes {
            windows.passed(decision.player);
        }
        answers += 1;
        observe(&mut checker, &engine).map_err(|v| fail(v.into(), engine.events()))?;
    }
    finish(&engine, checker.as_mut(), answers, auto_passes)
        .map_err(|kind| fail(kind, engine.events()))
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
    checker: Option<&mut InvariantChecker>,
    answers: usize,
    auto_passes: usize,
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
        checker.full_check(state)?;
        checker.check_full_replay(state, engine.events())?;
    }
    Ok(GameOutcome {
        result: *result,
        answers,
        auto_passes,
        turns: state.turn().number(),
        final_hash: state.state_hash(),
        events: engine.events().to_vec(),
    })
}
