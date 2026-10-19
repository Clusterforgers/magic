//! Tokens: what the `TokenCreated` event says (creation, copies and the predefined tokens are task (g)).

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::fold::invalid;
use crate::state::GameState;

/// A token that was created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TokenCreated {
    /// The token's object.
    pub object: ObjectId,
    /// Who controls it.
    pub controller: PlayerId,
    /// What it is called.
    pub name: String,
}

/// Folds the event; the engine has no tokens yet.
pub(crate) fn fold(_state: &mut GameState, _event: &TokenCreated) -> Result<(), EngineError> {
    Err(invalid("tokens are not in the engine yet"))
}
