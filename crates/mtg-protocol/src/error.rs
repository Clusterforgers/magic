//! Errors raised while building protocol payloads.

use mtg_engine::EngineError;
use thiserror::Error;

/// Why a payload could not be built.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// The events do not follow from the state they were projected against.
    #[error("events do not follow from the given state: {0}")]
    Events(#[from] EngineError),
}
