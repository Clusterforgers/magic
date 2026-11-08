//! The event that changes the targets of a spell or ability on the stack (CR 115.7).

use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use crate::choose::{Choices, Target};
use crate::error::EngineError;
use crate::fold::invalid;
use crate::object::ObjectRef;
use crate::state::{GameState, Tally};
use crate::zones::Zone;

/// A spell or ability on the stack has other targets: the same object, with what it announced and the new targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TargetsChanged {
    /// The spell or ability.
    pub object: ObjectRef,
    /// The player who controls it.
    pub controller: PlayerId,
    /// What it announced, with the new targets.
    #[serde(default, skip_serializing_if = "Choices::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Choices>", optional))]
    pub choices: Choices,
    /// The targets it did not have before, which count as targeted now.
    pub added: Vec<Target>,
}

/// Folds the event: the object keeps its place on the stack with its new choices.
pub(crate) fn fold(state: &mut GameState, change: &TargetsChanged) -> Result<(), EngineError> {
    let object = state
        .objects
        .get_mut(&change.object.id)
        .filter(|object| object.counter() == change.object.counter && object.zone() == Zone::Stack)
        .ok_or_else(|| invalid(format!("{:?} is not on the stack", change.object)))?;
    let face = object.choices.face;
    object.choices = change.choices.clone();
    object.choices.face = face;
    for target in &change.added {
        if let Target::Object(named) = target {
            state.tally_object(*named, Some(change.controller), Tally::Targeted);
        }
    }
    Ok(())
}
