//! `Effect::Regenerate`: permanents get a regeneration shield until end of turn (CR 701.19).

use mtg_cards::effect::Effect;
use mtg_cards::modify::Duration;
use mtg_core::ids::EffectId;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::rules::{Rule, RuleEffect};
use crate::zones::Zone;

/// Each permanent `what` names that is still the object named and on the battlefield gets a shield that lasts until the
/// end of the turn; a permanent that has left, a card elsewhere and a name that repeats get none. A shield is used
/// by the next destruction of the permanent, so two shields on one permanent take two destructions.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Regenerate { what } = effect else {
        return Err(wrong_effect("Regenerate"));
    };
    let state = cx.state();
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .zip(state.next_effect..)
        .map(|(object, id)| Event::RuleCreated {
            effect: RuleEffect {
                id: EffectId(id),
                rule: Rule::Regenerate(object.reference()),
                duration: Duration::UntilEndOfTurn,
                source: Some(cx.scope.source),
                controller: cx.controller(),
                turn: state.turn().number(),
            },
        })
        .collect();
    Ok(Run::events(events))
}
