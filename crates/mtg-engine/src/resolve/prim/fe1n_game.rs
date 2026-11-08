//! `Effect::LoseGame` and `Effect::DrawGame`: a player loses the game (CR 104.3), or the game is a draw (CR 104.4).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::player::LossReason;
use crate::resolve::{Cx, ResolveError, Run};

/// Why a player left through an effect.
const EFFECT_LOSS: &str = "an effect made the player lose the game";
/// Why every player left when the game was drawn.
const DRAWN: &str = "the game is a draw";

/// Each player the effect names who is in the game loses it, unless an effect says they can't lose, in turn order from
/// the active player.
pub(super) fn lose(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::LoseGame { who } = effect else {
        return Err(wrong_effect("LoseGame"));
    };
    let state = cx.state();
    let named = cx.scope.players(who)?;
    let events = state
        .turn_order_from(state.turn().active())
        .into_iter()
        .filter(|player| named.contains(player))
        .filter(|player| !crate::w17b_statics::w17b_cant_lose(state, *player))
        .map(|player| Event::PlayerLost {
            player,
            reason: LossReason::Format(EFFECT_LOSS.into()),
        })
        .collect();
    Ok(Run::events(events))
}

/// Every player in the game leaves it at once, in turn order from the active player, so none is left and the engine
/// ends the game as a draw. A draw is no loss, so "can't lose" does not stop it.
pub(super) fn draw(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::DrawGame {} = effect else {
        return Err(wrong_effect("DrawGame"));
    };
    let state = cx.state();
    let events = state
        .turn_order_from(state.turn().active())
        .into_iter()
        .map(|player| Event::PlayerLost {
            player,
            reason: LossReason::Format(DRAWN.into()),
        })
        .collect();
    Ok(Run::events(events))
}
