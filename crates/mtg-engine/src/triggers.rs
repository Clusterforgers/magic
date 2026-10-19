//! Triggered abilities: what the `AbilityTriggered` event says (detection, the queue and APNAP order are task (i)).

use serde::{Deserialize, Serialize};

use crate::abilities::Cause;
use crate::error::EngineError;
use crate::fold::invalid;
use crate::object::ObjectRef;
use crate::state::GameState;

/// A triggered ability that triggered and waits to be put on the stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Triggered {
    /// The object with the ability, as it was.
    pub source: ObjectRef,
    /// Its index among the rules of the source's card.
    pub ability: usize,
    /// What it triggered on.
    pub cause: Cause,
}

/// Folds the event; the queue it joins does not exist yet.
pub(crate) fn fold(_state: &mut GameState, _event: &Triggered) -> Result<(), EngineError> {
    Err(invalid("triggered abilities are not in the engine yet"))
}
