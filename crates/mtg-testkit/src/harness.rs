//! Runs bot games to completion while checking every invariant, failing with the seed and log.

use std::fmt;

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::manual::ManualAction;
use mtg_engine::state::{GameState, Stage};
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine, EngineError, Event, GameResult, GameSetup};
use thiserror::Error;

use crate::bot::{RandomBot, apply_answer};
use crate::invariants::{InvariantChecker, Violation};
use crate::manual_script::ManualScript;
use crate::watchdog::Watchdog;
use crate::windows::{Mode, SeatWindows, Windows};

/// Whole-state invariants run every this many answers; per-event checks run on every event.
const FULL_CHECK_EVERY: usize = 25;

/// Permanents on the table above which the bots only pass priority (see [`crowded`]); the fullest table of the suites
/// holds about 250 and the watchdog stops a game at [`crate::watchdog::MAX_PERMANENTS`].
pub const CROWDED_TABLE: usize = 300;

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
    /// Take a scripted manual action ([`ManualScript`]) at one priority decision in this many, in place of the bot's
    /// answer; 0 never. The bots themselves never take one.
    pub manual_every: usize,
    /// Take an out-of-band manual action (`Engine::manual`) by a random seat in the game before one decision in this
    /// many, of any kind and whoever it belongs to; 0 never.
    pub manual_anytime_every: usize,
    /// At every priority decision, try each offered play this many times on a copy of the game ([`crate::offers`])
    /// and fail with [`FailureKind::Unanswered`] when one cannot be taken; 0 never.
    pub check_offers: u64,
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
            manual_every: 0,
            manual_anytime_every: 0,
            check_offers: 0,
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
    /// How many times the engine asked a player to order replacement effects.
    pub replacement_asks: usize,
    /// How many times the engine asked a player to pick one of several options (a colour, a type, the damage a shield takes first).
    pub option_asks: usize,
    /// How many manual actions the script took ([`PlayOptions::manual_every`]); they count in `answers` too.
    pub manual_actions: usize,
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
    /// A priority window offered plays that end with nothing done whichever way they are answered.
    #[error("unanswered plays: {0}")]
    Unanswered(String),
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
    let mut bots = bots_for(setup.players.len(), seed, options);
    let mut watchdog = Watchdog::for_game(seed, &setup);
    let mut engine = Engine::new(setup).map_err(|e| fail(e.into(), &[]))?;
    observe(&mut checker, &engine).map_err(|v| fail(v.into(), engine.events()))?;

    let mut windows = table_windows(&mut bots, &engine, options);
    let pass = Answer::Priority(PriorityAction::Pass);
    let mut script = ManualScript::new(seed, options.manual_every);
    let mut anytime = ManualScript::anytime(seed, options.manual_anytime_every);
    let mut manual_actions = 0;
    let mut answers = 0;
    let mut auto_passes = 0;
    let mut replacement_asks = 0;
    let mut option_asks = 0;
    while let Some(decision) = engine.decision().cloned() {
        if answers >= options.max_answers {
            return Err(fail(FailureKind::StepLimit(answers), engine.events()));
        }
        let Some(bot) = bots.get_mut(decision.player.index()) else {
            let message = format!("decision for unknown {:?}", decision.player);
            return Err(fail(FailureKind::BadEnding(message), engine.events()));
        };
        if let DecisionKind::ChooseReplacement(options) = &decision.kind {
            // The order is asked only when two or more effects apply (CR 616.1).
            if options.candidates.len() < 2 {
                let message =
                    "an order of replacement effects was asked of fewer than two".to_owned();
                return Err(fail(FailureKind::BadEnding(message), engine.events()));
            }
            replacement_asks += 1;
        }
        if matches!(decision.kind, DecisionKind::ChooseOption(_)) {
            option_asks += 1;
        }
        if let Some((seat, action)) = out_of_band(&mut anytime, &engine) {
            let events = engine
                .manual(seat, &action)
                .map_err(|e| fail(e.into(), engine.events()))?;
            windows.observe(&events);
            manual_actions += 1;
            observe(&mut checker, &engine).map_err(|v| fail(v.into(), engine.events()))?;
            continue;
        }
        check_offers(&engine, seed, options.check_offers).map_err(|kind| fail(kind, &[]))?;
        let table_passes = windows.passes(engine.state(), &decision);
        let scripted = scripted_action(&mut script, &engine, &decision, table_passes);
        let answer = if table_passes {
            auto_passes += 1;
            pass.clone()
        } else if let Some(action) = scripted {
            manual_actions += 1;
            Answer::Priority(PriorityAction::Manual(action))
        } else if crowded(engine.state(), &decision) {
            pass.clone()
        } else {
            bot.choose_in(&decision, engine.state())
        };
        let seat_passes = !table_passes && answer == pass;
        let given = answer.clone();
        let events = apply_answer(&mut engine, &decision, answer)
            .map_err(|e| fail(e.into(), engine.events()))?;
        windows.observe(&events);
        if seat_passes {
            windows.passed(decision.player);
        }
        answers += 1;
        watchdog.answered(
            engine.state(),
            decision.player,
            decision.kind.name(),
            &given,
        );
        observe(&mut checker, &engine).map_err(|v| fail(v.into(), engine.events()))?;
    }
    finish(&engine, checker.as_mut(), answers, auto_passes)
        .map(|outcome| GameOutcome {
            replacement_asks,
            option_asks,
            manual_actions,
            ..outcome
        })
        .map_err(|kind| fail(kind, engine.events()))
}

/// The windows of the table: every seat in Full Control, passed for by the table (`auto_pass`) or set by its bot.
fn table_windows(bots: &mut [RandomBot], engine: &Engine, options: PlayOptions) -> Windows {
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
    windows
}

/// Whether `decision` is a priority decision at a table so full that its bots stop adding to it: a player may always
/// pass (CR 117.3d), and attacks, blocks and choices stay random. A board that doubles its tokens every round
/// (Rhys the Redeemed) otherwise grows without end, and every action of the engine costs more than the last.
fn crowded(state: &GameState, decision: &Decision) -> bool {
    matches!(decision.kind, DecisionKind::Priority(_))
        && state.zone(Zone::Battlefield).len() > CROWDED_TABLE
}

/// One seeded bot for each seat, set up as the options say.
fn bots_for(seats: usize, seed: u64, options: PlayOptions) -> Vec<RandomBot> {
    (0u64..)
        .take(seats)
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
        .collect()
}

/// The out-of-band action a random seat takes before this decision, if the script takes one now.
fn out_of_band(script: &mut ManualScript, engine: &Engine) -> Option<(PlayerId, ManualAction)> {
    if engine.replacing() || engine.state().stage() != Stage::Turns || !script.due() {
        return None;
    }
    let seat = script.seat(engine.state())?;
    script
        .action(engine.state(), seat)
        .map(|action| (seat, action))
}

/// The manual action the script takes at this decision, if it takes one: only a priority decision the table does not
/// pass for gets one.
fn scripted_action(
    script: &mut ManualScript,
    engine: &Engine,
    decision: &Decision,
    table_passes: bool,
) -> Option<ManualAction> {
    (!table_passes && matches!(decision.kind, DecisionKind::Priority(_)) && script.due())
        .then(|| script.action(engine.state(), decision.player))
        .flatten()
}

fn observe(checker: &mut Option<InvariantChecker>, engine: &Engine) -> Result<(), Violation> {
    match checker {
        Some(checker) => checker.observe_engine(engine),
        None => Ok(()),
    }
}

/// Whether the players still in the game explain the result: the last one standing wins and nobody left is a draw;
/// under the house rules of the turn limit and the winning life total a game also ends with several players in it, the
/// winner then alone at the highest life and a draw a tie for it or everyone at the winning life.
fn result_explained(state: &GameState, result: GameResult) -> bool {
    let survivors: Vec<_> = state.players().iter().filter(|p| p.is_in_game()).collect();
    let rules = state.house_rules();
    let highest = survivors.iter().map(|p| p.life()).max();
    let leaders: Vec<_> = survivors
        .iter()
        .filter(|p| Some(p.life()) == highest)
        .collect();
    let limited = rules
        .turn_limit
        .is_some_and(|limit| state.turn().round() >= limit);
    let everyone_wins = rules
        .winning_life
        .is_some_and(|goal| survivors.iter().all(|p| p.life() >= goal));
    match (result, survivors.as_slice()) {
        (GameResult::Winner(winner), [only]) => winner == only.id(),
        (GameResult::Draw, []) => true,
        (GameResult::Winner(winner), _) => {
            limited && matches!(leaders.as_slice(), [only] if only.id() == winner)
        }
        (GameResult::Draw, _) => (limited && leaders.len() > 1) || everyone_wins,
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
    if !result_explained(state, *result) {
        return Err(bad("result does not match the players still in the game"));
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
        replacement_asks: 0,
        option_asks: 0,
        manual_actions: 0,
    })
}

/// Fails with the plays of the priority decision of `engine` that cannot be taken in `tries` tries ([`crate::offers`]).
fn check_offers(engine: &Engine, seed: u64, tries: u64) -> Result<(), FailureKind> {
    if tries == 0 {
        return Ok(());
    }
    let unanswered = crate::offers::unanswered_plays(engine, seed, tries);
    if unanswered.is_empty() {
        return Ok(());
    }
    let lines: Vec<String> = unanswered.iter().map(ToString::to_string).collect();
    Err(FailureKind::Unanswered(lines.join("; ")))
}
