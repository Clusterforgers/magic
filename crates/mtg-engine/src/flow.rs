//! The engine's control loop as a pure function of state: what happens next?

use crate::announce::Pending;
use crate::commanders;
use crate::decision::Decision;
use crate::error::EngineError;
use crate::events::Event;
use crate::exile_return;
use crate::legend;
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

/// The objects of players who left first (CR 800.4a), then a resolution (state-based actions wait for it to end,
/// CR 704.3), then the end of the game when one player is left (CR 104.2a), then state-based actions to a fixpoint, then the game-end check, then the legend rule, then the state triggers whose state is true (CR 603.8), then the triggered
/// abilities waiting for the stack (CR 603.3), then the turn structure.
fn next_in_turns(state: &GameState) -> Progress {
    let departing = sba::departures_first(state);
    if !departing.is_empty() {
        return Progress::Events(departing);
    }
    if state.resolving().is_some() {
        return resolve::next(state);
    }
    if let Some(ended) = sba::game_over(state) {
        return Progress::Events(vec![ended]);
    }
    // A card exiled until a permanent leaves returns the moment it has left, before anything is checked (CR 610.3).
    let returning = exile_return::returns(state);
    if !returning.is_empty() {
        return Progress::Events(returning);
    }
    let actions = sba::check(state);
    if !actions.is_empty() {
        return Progress::Events(actions);
    }
    if let Some(progress) = settle(state) {
        return progress;
    }
    // The legend rule is a state-based action that asks (CR 704.5j); it waits for the end of the game and the removal of
    // the objects of players who left, so it never asks about a permanent that is gone.
    if let Some(decision) = legend::decision(state) {
        return Progress::Decide(decision);
    }
    // So is the owner's choice to put a commander from a graveyard or exile into the command zone (CR 903.9a).
    if let Some(decision) = commanders::decision(state) {
        return Progress::Decide(decision);
    }
    // A state is true where a player would receive priority, after state-based actions (CR 603.8).
    let states = triggers::state_triggers(state);
    if !states.is_empty() {
        return Progress::Events(
            states
                .into_iter()
                .map(|trigger| Event::AbilityTriggered(Box::new(trigger)))
                .collect(),
        );
    }
    triggers::sweep(state).unwrap_or_else(|| steps::next(state))
}

/// The game-end check, then the removal of eliminated players' objects and ended effects.
fn settle(state: &GameState) -> Option<Progress> {
    if let Some(result) = state.ruleset().game_result(state) {
        return Some(Progress::Events(vec![Event::GameEnded { result }]));
    }
    let housekeeping = sba::housekeeping(state);
    (!housekeeping.is_empty()).then_some(Progress::Events(housekeeping))
}
