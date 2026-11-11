//! `Effect::LoseUnspentMana`: players lose all unspent mana (CR 106.4).

use mtg_cards::effect::Effect;
use mtg_cards::spend::ManaLasts;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// Each player the effect names who is still in the game and has mana in their pool loses all of it, the mana that
/// lasts until the end of the turn included; an empty pool is no event.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::LoseUnspentMana { who } = effect else {
        return Err(wrong_effect("LoseUnspentMana"));
    };
    let mut events = Vec::new();
    for player in cx.scope.players(who)? {
        let holds = cx
            .state()
            .player(player)
            .is_some_and(|seat| seat.mana().total() > 0);
        if cx.state().is_alive(player) && holds {
            events.push(Event::ManaPoolEmptied {
                player,
                through: ManaLasts::Turn,
                kept: Vec::new(),
                becomes: None,
            });
        }
    }
    Ok(Run::events(events))
}
