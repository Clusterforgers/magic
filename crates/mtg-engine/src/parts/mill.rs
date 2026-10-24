//! Milling as a cost (CR 701.17): the top cards of the payer's library go to their graveyard.

use mtg_core::ids::{ObjectId, PlayerId};

use super::Payer;
use crate::events::{Event, ZoneCause};
use crate::library::top_cards;
use crate::state::GameState;
use crate::zones::Zone;

/// The cards the payer's library holds.
pub(super) fn most(payer: &Payer<'_>) -> u32 {
    let held = payer.state.zone(Zone::Library(payer.player)).len();
    u32::try_from(held).unwrap_or(u32::MAX)
}

/// Whether the library holds that many cards: a cost cannot mill what is not there.
pub(super) fn payable(payer: &Payer<'_>, amount: u32) -> bool {
    most(payer) >= amount
}

/// The top cards of the library, the top card first, each a move to the graveyard.
pub(super) fn events(state: &GameState, player: PlayerId, amount: u32) -> Vec<Event> {
    let wanted = usize::try_from(amount).unwrap_or(usize::MAX);
    top_cards(state, player, wanted)
        .into_iter()
        .map(|object| milled(player, object))
        .collect()
}

/// The events of a payment whose sources each mill from the same library: each source worked out its mill from the
/// library as it stands before the payment, so the mills are given the cards in turn, the top card to the first, and a
/// mill that finds the library empty mills nothing.
pub(crate) fn in_turn(state: &GameState, player: PlayerId, events: Vec<Event>) -> Vec<Event> {
    let from = Zone::Library(player);
    let is_mill = |event: &Event| matches!(event, Event::ZoneChanged { from: found, cause: ZoneCause::Milled, .. } if *found == from);
    let total = events.iter().filter(|event| is_mill(event)).count();
    if total < 2 {
        return events;
    }
    let mut cards = top_cards(state, player, total).into_iter();
    events
        .into_iter()
        .filter_map(|event| {
            if is_mill(&event) {
                cards.next().map(|object| milled(player, object))
            } else {
                Some(event)
            }
        })
        .collect()
}

/// One card of the library milled.
pub(crate) const fn milled(player: PlayerId, object: ObjectId) -> Event {
    Event::ZoneChanged {
        object,
        from: Zone::Library(player),
        to: Zone::Graveyard(player),
        cause: ZoneCause::Milled,
    }
}
