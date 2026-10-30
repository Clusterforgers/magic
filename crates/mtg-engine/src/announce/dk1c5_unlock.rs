//! Unlocking a door of a Room (CR 709.5e): a special action that does not use the stack. The announcement is only the
//! payment of the door's mana cost, which can be backed out of; the event that follows unlocks the door.

use mtg_core::ids::{ObjectId, PlayerId};

use super::{Next, suspend};
use crate::dk1c5_rooms::{dk1c5_door, dk1c5_events};
use crate::priority::sorcery_timing;
use crate::state::GameState;

/// The payment decision of unlocking `door` of `card`, or nothing when `player` cannot unlock it now.
pub(super) fn next(state: &GameState, player: PlayerId, card: ObjectId, door: u8) -> Next {
    let Some(cost) = dk1c5_door(state, card, door).map(|face| face.mana_cost.clone().unwrap_or_default())
    else {
        return Next::Drop;
    };
    if !sorcery_timing(state, player) || dk1c5_events(state, player, card, door).is_none() {
        return Next::Drop;
    }
    suspend::pay(state, player, card, cost, || {
        dk1c5_events(state, player, card, door)
    })
}
