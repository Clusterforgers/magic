//! `Effect::Rule`: a rule of the game changes for a while (CR 611).

use mtg_cards::effect::Effect;
use mtg_core::ids::EffectId;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::rules::{RuleEffect, lower_rule, names_nothing};

/// The rule begins to apply to the players and objects its references name now (CR 611.2c) and lasts for its duration.
/// Naming no player and no object begins nothing. A rule the engine does not play refuses by name.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Rule { rule, duration } = effect else {
        return Err(wrong_effect("Rule"));
    };
    let state = cx.state();
    let rule = lower_rule(&cx.scope, rule)?;
    if names_nothing(&rule) {
        return Ok(Run::events(Vec::new()));
    }
    Ok(Run::events(vec![Event::RuleCreated {
        effect: RuleEffect {
            id: EffectId(state.next_effect),
            rule,
            duration: duration.clone(),
            source: Some(cx.scope.source),
            controller: cx.controller(),
            turn: state.turn().number(),
        },
    }]))
}
