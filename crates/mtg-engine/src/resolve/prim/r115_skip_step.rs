//! `Effect::R115SkipStep`: the step a `ReplaceWhen::R115BeginStep` replacement is about does not begin, and the step
//! after it begins in its place (CR 614.10b): "If you would begin your draw step, you may skip that step instead."

use mtg_cards::effect::Effect;

use super::r306_add_step::engine_step;
use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// The step that would follow the skipped one, itself passing over the steps the turn skips, begins.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R115SkipStep { step } = effect else {
        return Err(wrong_effect("R115SkipStep"));
    };
    let next = crate::steps::r115_step_after(cx.state(), engine_step(*step));
    Ok(Run::events(vec![Event::StepBegan { step: next }]))
}
