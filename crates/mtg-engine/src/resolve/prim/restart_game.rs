//! `Effect::RestartGame`: the game restarts (CR 727), leaving the cards the effect keeps in exile.

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// One `GameRestarted` by the controller of the effect, keeping the exiled cards its selector picks.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::RestartGame { keep } = effect else {
        return Err(wrong_effect("RestartGame"));
    };
    let state = cx.state();
    let kept = cx
        .scope
        .objects(&Ref::Each(keep.clone()))?
        .into_iter()
        .map(|card| card.id)
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|card| card.zone() == Zone::Exile)
        })
        .collect();
    Ok(Run::events(vec![Event::GameRestarted {
        by: cx.controller(),
        kept,
    }]))
}
