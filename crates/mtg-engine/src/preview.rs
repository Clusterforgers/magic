//! Previewing a pending attack or block: the combat forecast as if the answer were declared, without declaring it.
//!
//! The answer takes the real path on a scratch copy of the state: `answers::handle` validates it and
//! builds its events, the replacement step (`replace_dry`) and `apply_event` carry them out, and
//! `pending_combat_forecast` reads the result, the damage steps ahead with their replacement effects. Nothing
//! is re-implemented, so a preview cannot disagree with the forecast after the real answer, and the game
//! itself (its log, its random stream, its state hash) is never touched. The cost is one clone of the state,
//! which the server already pays for every step.

use crate::answers::{Outcome, handle};
use crate::decision::{Answer, Decision, DecisionKind};
use crate::error::{EngineError, PreviewError};
use crate::fold::apply_event;
use crate::forecast::{CombatForecast, pending_combat_forecast};
use crate::replace_dry::{Picks, settle};
use crate::state::GameState;

/// The forecast after `answer` to `decision`, on a copy of `state`; see the module docs.
pub(crate) fn preview_forecast(
    state: &GameState,
    decision: &Decision,
    answer: &Answer,
) -> Result<CombatForecast, PreviewError> {
    if !matches!(
        decision.kind,
        DecisionKind::DeclareAttackers(_) | DecisionKind::DeclareBlockers(_)
    ) {
        return Err(PreviewError::NotPreviewable(decision.kind.name()));
    }
    let events = match handle(state, None, decision, answer.clone()) {
        Ok(Outcome::Events(events)) => events,
        Ok(_) => return Err(PreviewError::NotPreviewable(decision.kind.name())),
        Err(EngineError::Illegal(illegal)) => return Err(PreviewError::Illegal(illegal)),
        Err(other) => return Err(PreviewError::Engine(other)),
    };
    let mut scratch = state.clone();
    let events =
        settle(&mut scratch, &events, &mut Picks::default()).map_err(PreviewError::Engine)?;
    for event in &events {
        apply_event(&mut scratch, event).map_err(PreviewError::Engine)?;
    }
    pending_combat_forecast(&scratch).ok_or(PreviewError::NoCombat)
}
