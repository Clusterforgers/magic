//! `Effect::ExtraTurn`: players take extra turns after this one (CR 500.7).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// Each player the effect names, in turn order from the active player, gets `count` extra turns, added one at a time:
/// the latest added is taken first (CR 500.7), so the turns of the player named last are taken before the others. A
/// player who has left the game gets none.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ExtraTurn {
        who,
        count,
        skip_untap,
    } = effect
    else {
        return Err(wrong_effect("ExtraTurn"));
    };
    let turns = cx.scope.amount(count)?.max(0);
    let named = cx.scope.players(who)?;
    let mut events = Vec::new();
    for player in cx.state().turn_order_from(cx.state().turn().active()) {
        if named.contains(&player) {
            for _ in 0..turns {
                events.push(Event::ExtraTurnAdded {
                    player,
                    skip_untap: *skip_untap,
                });
            }
        }
    }
    Ok(Run::events(events))
}
