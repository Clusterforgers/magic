//! `ChooseReplacement`: which of several replacement effects applies first (CR 616.1).
//!
//! The replacement step asks it of the affected player when two or more effects apply to one event and none is a
//! self-replacement effect. The answer is logged as `ChoiceMade`'s `Choice::Replacement` only through the caller that
//! holds the batch: it joins the journal the step is rerun with (`replace::process`).

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use super::Choice;
use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;

/// One replacement effect that applies to the event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReplacementCandidate {
    /// The object with the effect.
    pub source: ObjectId,
    /// Its index among the source's rules.
    pub ability: usize,
}

/// The effects the affected player orders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReplacementOptions {
    /// The effects that apply, any of which may be chosen first.
    pub candidates: Vec<ReplacementCandidate>,
    /// The event they replace in words ("Create 2 token(s)."); empty when the engine gave none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub text: String,
}

/// The effect applied first, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReplacementChoice(pub usize);

/// Checks that the index names one of the effects offered and hands it on as the choice of `player`.
pub(crate) fn answer(
    player: PlayerId,
    options: &ReplacementOptions,
    choice: ReplacementChoice,
) -> Result<Outcome, EngineError> {
    if choice.0 >= options.candidates.len() {
        return Err(IllegalAnswer::NotOffered("replacement effect").into());
    }
    Ok(Outcome::Events(vec![Event::ChoiceMade {
        player,
        choice: Choice::Replacement(choice.0),
    }]))
}
