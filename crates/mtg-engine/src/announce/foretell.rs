//! Foretelling a card (CR 702.143a): a special action that does not use the stack. The announcement is only the payment
//! of {2}, which can be backed out of; the event that follows it exiles the card face down.

use mtg_core::ids::{ObjectId, PlayerId};

use super::{Next, suspend};
use crate::events::{Event, ZoneCause};
use crate::state::GameState;
use crate::ways;
use crate::zones::Zone;

/// The payment decision of the foretelling of `card`, or nothing when it cannot be foretold now.
pub(super) fn next(state: &GameState, player: PlayerId, card: ObjectId) -> Next {
    let Some(cost) = ways::foretell_action_cost(state, card) else {
        return Next::Drop;
    };
    if events(state, card).is_none() || state.turn().active() != player {
        return Next::Drop;
    }
    suspend::pay(state, player, card, cost, || events(state, card))
}

/// The events of foretelling `card`: it moves from the hand to exile, face down. They come between the life and the mana
/// of the payment.
pub(crate) fn events(state: &GameState, card: ObjectId) -> Option<Vec<Event>> {
    let object = state.object(card)?;
    let Zone::Hand(_) = object.zone() else {
        return None;
    };
    Some(vec![Event::ZoneChanged {
        object: card,
        from: object.zone(),
        to: Zone::Exile,
        cause: ZoneCause::Foretold,
    }])
}
