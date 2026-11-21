//! Ending an effect by paying for it (CR 116.2c): a special action that does not use the stack. The announcement is only
//! the payment of the price, which can be backed out of; the events that follow end every part of the effect.

use mtg_core::ids::{EffectId, PlayerId};

use super::{Next, suspend};
use crate::e15g_paid::{events, offer};
use crate::state::GameState;

/// The payment decision of ending `effect`, or nothing when `player` cannot end it now.
pub(super) fn next(state: &GameState, player: PlayerId, effect: EffectId) -> Next {
    let Some((object, cost)) = offer(state, effect) else {
        return Next::Drop;
    };
    if events(state, player, effect).is_none() {
        return Next::Drop;
    }
    suspend::pay(state, player, object, cost, || {
        events(state, player, effect)
    })
}

/// The payment decision of paying to do what the rule effect `effect` says, or nothing when `player` cannot now
/// (`r114_pay_to_do`).
pub(super) fn r114_next(state: &GameState, player: PlayerId, effect: EffectId) -> Next {
    let Some((object, cost)) = crate::r114_pay_to_do::offer(state, effect) else {
        return Next::Drop;
    };
    if crate::r114_pay_to_do::events(state, player, effect).is_none() {
        return Next::Drop;
    }
    suspend::pay(state, player, object, cost, || {
        crate::r114_pay_to_do::events(state, player, effect)
    })
}
