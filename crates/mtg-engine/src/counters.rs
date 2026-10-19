//! Counters: what the `CountersChanged` event says (storage and the effects that change them are tasks (f) and (k)).

use mtg_cards::value::CounterKind;
use serde::{Deserialize, Serialize};

use crate::choose::Target;
use crate::error::EngineError;
use crate::fold::invalid;
use crate::state::GameState;

/// Counters put on or removed from an object or a player.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CountersChanged {
    /// What holds the counters.
    pub on: Target,
    /// Their kind.
    pub counter: CounterKind,
    /// How many were added (positive) or removed (negative).
    pub delta: i32,
}

/// Folds the event; the engine has no counters yet.
pub(crate) fn fold(_state: &mut GameState, _event: &CountersChanged) -> Result<(), EngineError> {
    Err(invalid("counters are not in the engine yet"))
}
