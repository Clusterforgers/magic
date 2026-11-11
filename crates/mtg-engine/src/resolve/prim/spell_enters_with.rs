//! `Effect::SpellEntersWith`: a permanent spell enters with additional +1/+1 counters as it resolves (CR 614.1c).

use mtg_cards::effect::Effect;
use mtg_cards::modify::Duration;
use mtg_core::ids::EffectId;

use super::wrong_effect;
use crate::events::Event;
use crate::object::GameObject;
use crate::resolve::{Cx, ResolveError, Run};
use crate::rules::{Rule, RuleEffect};
use crate::zones::Zone;

/// Each spell the reference names that is still on the stack as the same object gets a rule that gives the permanent it
/// becomes the counters, worked out now; the rule is spent once the spell leaves the stack. Nothing happens for a spell
/// that has resolved or left, or for a count of zero or less.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::SpellEntersWith { spell, count } = effect else {
        return Err(wrong_effect("SpellEntersWith"));
    };
    let counters = u32::try_from(cx.scope.amount(count)?).unwrap_or(0);
    if counters == 0 {
        return Ok(Run::events(Vec::new()));
    }
    let state = cx.state();
    let events = cx
        .scope
        .live_objects(spell)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Stack)
        .map(GameObject::reference)
        .zip(0u64..)
        .map(|(named, at)| Event::RuleCreated {
            effect: Box::new(RuleEffect {
                id: EffectId(state.next_effect + at),
                rule: Rule::EntersWith {
                    spell: named,
                    counters,
                    keyword: None,
                    this_turn: false,
                },
                duration: Duration::UntilEndOfTurn,
                source: Some(cx.scope.source),
                controller: cx.controller(),
                turn: state.turn().number(),
            }),
        })
        .collect();
    Ok(Run::events(events))
}
