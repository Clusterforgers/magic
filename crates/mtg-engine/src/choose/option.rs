//! `ChooseOption`: one of a short list of things (a colour, a defending player, a name).
//!
//! The type exists; the task that asks it (`docs/cards-dsl.md`, Tasks (d)) fills in the check.

use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};

/// One thing to choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ChoiceOption {
    /// A player.
    Player(PlayerId),
    /// A colour.
    Color(Color),
    /// Anything else, by its name.
    Label(String),
}

/// The things a player may choose between.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OptionOptions {
    /// The object whose effect asks.
    pub source: ObjectId,
    /// The things to choose between.
    pub options: Vec<ChoiceOption>,
    /// What is chosen, in the words of the card ("Choose a color."); empty when the engine gave none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub text: String,
}

/// The option chosen, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OptionChoice(pub usize);

pub(crate) fn answer(
    _player: PlayerId,
    _options: &OptionOptions,
    _choice: OptionChoice,
) -> Result<Outcome, EngineError> {
    Err(IllegalAnswer::Unsupported("choose-option").into())
}
