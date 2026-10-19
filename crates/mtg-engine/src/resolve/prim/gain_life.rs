//! `Effect::GainLife`: players gain life (CR 119.2).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// Each player the effect names who is still in the game gains the amount; a gain of nothing is no event.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::GainLife { who, amount } = effect else {
        return Err(wrong_effect("GainLife"));
    };
    let amount = u32::try_from(cx.scope.amount(amount)?.max(0)).unwrap_or(u32::MAX);
    let mut events = Vec::new();
    if amount > 0 {
        for player in cx.scope.players(who)? {
            if cx.state().is_alive(player) {
                events.push(Event::LifeGained { player, amount });
            }
        }
    }
    Ok(Run::events(events))
}
