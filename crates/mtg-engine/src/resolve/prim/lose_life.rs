//! `Effect::LoseLife`: players lose life (CR 119.3); damage is its own effect.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::rules::life_total_cant_change;

/// Each player the effect names who is still in the game loses the amount; a loss of nothing is no event.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::LoseLife { who, amount } = effect else {
        return Err(wrong_effect("LoseLife"));
    };
    let amount = u32::try_from(cx.scope.amount(amount)?.max(0)).unwrap_or(u32::MAX);
    let mut events = Vec::new();
    if amount > 0 {
        for player in cx.scope.players(who)? {
            if cx.state().is_alive(player) && !life_total_cant_change(cx.state(), player) {
                events.push(Event::LifeLost { player, amount });
            }
        }
    }
    Ok(Run::events(events))
}
