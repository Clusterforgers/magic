//! `Effect::Draw`: players draw cards (CR 121).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::resolve::{Cx, ResolveError, Run};
use crate::steps::draw;

/// Each player the effect names draws, in turn order; a draw from an empty library is logged as such.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Draw { who, count } = effect else {
        return Err(wrong_effect("Draw"));
    };
    let cards = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let mut events = Vec::new();
    for player in cx.scope.players(who)? {
        if cx.state().is_alive(player) {
            events.extend(draw(cx.state(), player, cards));
        }
    }
    Ok(Run::events(events))
}
