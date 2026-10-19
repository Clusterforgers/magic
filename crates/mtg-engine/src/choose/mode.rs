//! `ChooseMode`: the modes of a modal spell or ability (CR 700.2).
//!
//! The type exists; the task that asks it (`docs/cards-dsl.md`, Tasks (d)) fills in the check.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};

/// One mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModeOption {
    /// Its index among the ability's modes.
    pub index: usize,
    /// A short name, when the card gives one.
    pub label: Option<String>,
}

/// The modes a player may choose and how many.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModeOptions {
    /// The spell or ability being announced.
    pub source: ObjectId,
    /// The modes that may be chosen.
    pub modes: Vec<ModeOption>,
    /// The fewest to choose.
    pub min: u32,
    /// The most to choose.
    pub max: u32,
    /// The same mode may be chosen more than once.
    pub repeat: bool,
    /// Whether the cast or activation may still be backed out of: nothing is logged until it is paid for. A triggered
    /// ability that chooses its modes as it goes on the stack cannot (CR 603.3c).
    pub can_cancel: bool,
}

/// The answer to a mode decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ModeChoice {
    /// These modes, by index, in the order they happen: between `min` and `max` of them.
    Modes(Vec<usize>),
    /// Back out of the cast or activation (when `can_cancel`).
    Cancel,
}

pub(crate) fn answer(
    _player: PlayerId,
    _options: &ModeOptions,
    _choice: ModeChoice,
) -> Result<Outcome, EngineError> {
    Err(IllegalAnswer::Unsupported("choose-mode").into())
}
