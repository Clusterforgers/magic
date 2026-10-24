//! `Effect::AddPhase`: phases are added after the current phase (CR 500.8).

use mtg_cards::effect::{Effect, PhaseKind};

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::turn::{Phase, Step};

/// The step an added phase begins with. An added main phase is a postcombat main phase (CR 505.1a).
const fn first_step(phase: PhaseKind) -> Step {
    match phase {
        PhaseKind::Beginning => Step::Untap,
        PhaseKind::Combat => Step::BeginCombat,
        PhaseKind::Main => Step::Main2,
    }
}

/// The phases begin after the current one, in the order the effect lists them and before any added earlier. Nothing is
/// added once the ending phase has begun, because no phase follows it, or before the first turn.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::AddPhase { phases } = effect else {
        return Err(wrong_effect("AddPhase"));
    };
    let turn = cx.state().turn();
    if turn.number() == 0 || turn.step().phase() == Phase::Ending || phases.is_empty() {
        return Ok(Run::events(Vec::new()));
    }
    Ok(Run::events(vec![Event::PhasesAdded {
        steps: phases.iter().map(|phase| first_step(*phase)).collect(),
    }]))
}
