//! `Effect::Revive`: a Spirit returns to life ([`crate::spirits`]).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::resolve::{Cx, ResolveError, Run};

/// Each player named who is a Spirit loses its haunt emblems, returns with the life total and draws the cards; anyone
/// else is unchanged.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Revive { who, life, draw } = effect else {
        return Err(wrong_effect("Revive"));
    };
    let life = i32::try_from(cx.scope.amount(life)?.max(1)).unwrap_or(i32::MAX);
    let cards = usize::try_from(cx.scope.amount(draw)?.max(0)).unwrap_or(usize::MAX);
    let mut events = Vec::new();
    for player in cx.scope.players(who)? {
        events.extend(crate::spirits::revive(cx.state(), player, life, cards));
    }
    Ok(Run::events(events))
}
