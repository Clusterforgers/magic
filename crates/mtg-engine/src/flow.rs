//! The engine's control loop as a pure function of state: what happens next?

use crate::decision::Decision;
use crate::events::Event;
use crate::mulligan;
use crate::sba;
use crate::state::{GameState, Stage};
use crate::steps;

/// What the engine does next.
#[derive(Debug)]
pub(crate) enum Progress {
    /// State changes to fold; ask again afterwards.
    Events(Vec<Event>),
    /// A player must choose.
    Decide(Decision),
    /// The game is over.
    Done,
}

/// Computes the next batch of events or the next decision from `state` alone.
pub(crate) fn next(state: &GameState) -> Progress {
    match state.stage() {
        Stage::PreGame => Progress::Events(mulligan::pregame(state)),
        Stage::Mulligan => mulligan::next(state),
        Stage::Finished => Progress::Done,
        Stage::Turns => next_in_turns(state),
    }
}

/// State-based actions first, to a fixpoint, then the game-end check, then the turn structure.
fn next_in_turns(state: &GameState) -> Progress {
    let actions = sba::check(state);
    if !actions.is_empty() {
        return Progress::Events(actions);
    }
    if let Some(result) = state.format().game_result(state) {
        return Progress::Events(vec![Event::GameEnded { result }]);
    }
    let housekeeping = sba::housekeeping(state);
    if !housekeeping.is_empty() {
        return Progress::Events(housekeeping);
    }
    steps::next(state)
}
