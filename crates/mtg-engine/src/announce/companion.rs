//! Putting a companion into the hand (CR 702.139): a special action that does not use the stack. The announcement is only
//! the payment of {3}, which can be backed out of; the event that follows it brings the card into the game.

use mtg_core::ids::{ObjectId, PlayerId};

use super::{Next, suspend};
use crate::companions;
use crate::events::Event;
use crate::priority::sorcery_timing;
use crate::state::GameState;

/// The payment decision of bringing the companion `card` of `player` into the game, or nothing when it cannot be done now.
pub(super) fn next(state: &GameState, player: PlayerId, card: ObjectId) -> Next {
    if companions::outside(state, player) != Some(card) || !sorcery_timing(state, player) {
        return Next::Drop;
    }
    suspend::pay(state, player, card, companions::action_cost(), || {
        events(state, player)
    })
}

/// The events of bringing the companion of `player` into their hand. They come between the life and the mana of the
/// payment.
pub(crate) fn events(state: &GameState, player: PlayerId) -> Option<Vec<Event>> {
    companions::events(state, player)
}
