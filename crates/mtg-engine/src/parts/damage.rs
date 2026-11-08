//! Damage to the payer as a cost ("{CARDNAME} deals 2 damage to you"): the source deals it, so lifelink and prevention
//! apply (CR 120.3).

use mtg_core::ids::{ObjectId, PlayerId};

use crate::combat::DamageTarget;
use crate::damage::{Dealer, dealt_by};
use crate::events::Event;
use crate::state::GameState;

/// The damage the source deals to the payer; none when the source is gone.
pub(super) fn events(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    amount: u32,
) -> Vec<Event> {
    if state.object(source).is_none() {
        return Vec::new();
    }
    dealt_by(
        state,
        &Dealer::live(state, source),
        DamageTarget::Player(player),
        amount,
    )
}
