//! Previewing a target answer: the stack forecast of the spell being cast as if it resolved with those targets.
//!
//! The answer goes through [`Engine::apply`] on a copy of the engine, so it is validated exactly as a real one. The
//! spell's announcement then waits for the rest (its payment, most often); the copy puts the spell on the stack with what
//! was announced, without paying, and the stack forecast reads it. The engine itself is never touched.

use mtg_core::ids::PlayerId;

use super::Engine;
use crate::announce::What;
use crate::decision::{Answer, DecisionKind};
use crate::error::{EngineError, PreviewError};
use crate::fold::apply_event;
use crate::replace_dry::{Picks, settle};
use crate::stack_forecast::{ForecastChange, stack_forecast};

impl Engine {
    /// What the spell `seat` is casting would change if it resolved with the targets of `answer`, the answer to the
    /// `ChooseTargets` decision it faces now, without giving it; empty when the stack forecast has nothing to say.
    ///
    /// Only the targets of a spell can be previewed. The engine is not touched: no event is logged, no random number is
    /// drawn and the state hash stays as it is.
    pub fn preview_targets(
        &self,
        seat: PlayerId,
        answer: &Answer,
    ) -> Result<Vec<ForecastChange>, PreviewError> {
        let decision = self.decision.as_ref().ok_or(PreviewError::NoDecision)?;
        if self.shown.as_ref().map(|shown| shown.player) != Some(seat) {
            return Err(PreviewError::NotYourDecision(seat));
        }
        let spell = match (&decision.kind, self.pending.as_ref().map(|p| &p.what)) {
            (DecisionKind::ChooseTargets(_), Some(What::Spell(spell))) => *spell,
            _ => return Err(PreviewError::NotPreviewable(decision.kind.name())),
        };
        let mut probe = self.clone();
        probe.apply(answer.clone()).map_err(|error| match error {
            EngineError::Illegal(illegal) => PreviewError::Illegal(illegal),
            other => PreviewError::Engine(other),
        })?;
        let mut scratch = probe.state.clone();
        if let Some(pending) = probe
            .pending
            .as_ref()
            .filter(|p| p.what == What::Spell(spell))
        {
            let cast = crate::cast::spell_cast(pending.player, spell, &pending.choices, false);
            let events =
                settle(&mut scratch, &cast, &mut Picks::default()).map_err(PreviewError::Engine)?;
            for event in &events {
                apply_event(&mut scratch, event).map_err(PreviewError::Engine)?;
            }
        }
        Ok(stack_forecast(&scratch).unwrap_or_default())
    }
}
