//! The engine's control loop as a pure function of state: what happens next?

use crate::cast::Pending;
use crate::decision::Decision;
use crate::error::EngineError;
use crate::events::Event;
use crate::mulligan;
use crate::resolve;
use crate::sba;
use crate::state::{GameState, Stage};
use crate::steps;
use crate::triggers;

/// What the engine does next.
#[derive(Debug)]
pub(crate) enum Progress {
    /// State changes to fold; ask again afterwards.
    Events(Vec<Event>),
    /// A player must choose.
    Decide(Decision),
    /// A triggered ability goes on the stack next: its targets are announced like those of a cast.
    Put(Pending),
    /// The game is over.
    Done,
    /// The game cannot go on: a spell or ability uses something the engine does not play.
    Fail(EngineError),
}

/// Computes the next batch of events or the next decision from `state` alone.
pub(crate) fn next(state: &GameState) -> Progress {
    match state.stage() {
        Stage::PreGame => Progress::Events(mulligan::pregame(state)),
        // A player can concede during the opening, so it needs the same end-of-game handling as turns.
        Stage::Mulligan => settle(state).unwrap_or_else(|| mulligan::next(state)),
        Stage::Finished => Progress::Done,
        Stage::Turns => next_in_turns(state),
    }
}

/// A resolution first (state-based actions wait for it to end, CR 704.3), then state-based actions to a
/// fixpoint, then the game-end check, then the triggered abilities waiting for the stack (CR 603.3), then the turn
/// structure.
fn next_in_turns(state: &GameState) -> Progress {
    if state.resolving().is_some() {
        return resolve::next(state);
    }
    let actions = sba::check(state);
    if !actions.is_empty() {
        return Progress::Events(actions);
    }
    if let Some(progress) = settle(state) {
        return progress;
    }
    triggers::sweep(state).unwrap_or_else(|| steps::next(state))
}

/// The game-end check, then the removal of eliminated players' objects and ended effects.
fn settle(state: &GameState) -> Option<Progress> {
    if let Some(result) = state.format().game_result(state) {
        return Some(Progress::Events(vec![Event::GameEnded { result }]));
    }
    let housekeeping = sba::housekeeping(state);
    (!housekeeping.is_empty()).then_some(Progress::Events(housekeeping))
}
