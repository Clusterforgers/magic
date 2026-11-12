//! `Effect::AddStep`: a step is added right after the current step ("you get an additional upkeep step after this step").

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::turn::Step;

/// The engine's step for the step a card names (the two enums list the same thirteen steps).
const fn engine_step(step: mtg_cards::trigger::Step) -> Step {
    use mtg_cards::trigger::Step as Named;
    match step {
        Named::Untap => Step::Untap,
        Named::Upkeep => Step::Upkeep,
        Named::Draw => Step::Draw,
        Named::Main1 => Step::Main1,
        Named::BeginCombat => Step::BeginCombat,
        Named::DeclareAttackers => Step::DeclareAttackers,
        Named::DeclareBlockers => Step::DeclareBlockers,
        Named::FirstStrikeDamage => Step::FirstStrikeDamage,
        Named::CombatDamage => Step::CombatDamage,
        Named::EndCombat => Step::EndCombat,
        Named::Main2 => Step::Main2,
        Named::End => Step::End,
        Named::Cleanup => Step::Cleanup,
    }
}

/// The step begins when the current one ends, before any step added earlier. Nothing is added before the first turn.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::AddStep { step } = effect else {
        return Err(wrong_effect("AddStep"));
    };
    if cx.state().turn().number() == 0 {
        return Ok(Run::events(Vec::new()));
    }
    Ok(Run::events(vec![Event::StepAdded {
        step: engine_step(*step),
    }]))
}
