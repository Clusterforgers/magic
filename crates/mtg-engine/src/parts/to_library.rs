//! Putting cards from a zone of the payer into their library as a cost, on top or at the bottom.

use mtg_cards::refs::{Selector, ZoneKind};
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::characteristics::controller_of;
use crate::events::{Event, ZoneCause};
use crate::object::ObjectKind;
use crate::state::GameState;
use crate::zones::Zone;

/// Whether the part can take cards from this zone: the hand, the graveyard and the battlefield.
pub(super) const fn supported(from: ZoneKind) -> bool {
    matches!(
        from,
        ZoneKind::Hand | ZoneKind::Graveyard | ZoneKind::Battlefield
    )
}

/// The zone of the payer the cards come from.
const fn zone_of(from: ZoneKind, player: mtg_core::ids::PlayerId) -> Zone {
    match from {
        ZoneKind::Hand => Zone::Hand(player),
        ZoneKind::Graveyard => Zone::Graveyard(player),
        _ => Zone::Battlefield,
    }
}

/// The cards of the zone that the payer holds or controls and that match; a token is not a card of a library.
pub(super) fn candidates(payer: &Payer<'_>, from: ZoneKind, filter: &Selector) -> Vec<ObjectId> {
    let zone = zone_of(from, payer.player);
    payer.matching(payer.state.zone(zone).iter().copied(), filter, |id| {
        (zone != Zone::Battlefield || controller_of(payer.state, id) == Some(payer.player))
            && payer
                .state
                .object(id)
                .is_some_and(|object| object.kind() == ObjectKind::Card)
    })
}

/// "Put a card from your hand on top of your library."
pub(super) fn text(from: ZoneKind, filter: &Selector, count: usize, bottom: bool) -> String {
    let place = if bottom { "the bottom" } else { "the top" };
    let taken = match from {
        ZoneKind::Hand => crate::wording::exile_from_hand(filter, count),
        ZoneKind::Graveyard => crate::wording::exile_from_graveyard(filter, count),
        _ => crate::wording::return_to_hand(filter, count),
    };
    let what = taken.trim_end_matches('.').replacen("Exile", "Put", 1);
    let what = what.replacen("Return", "Put", 1);
    let what = what.split(" to its owner").next().unwrap_or(&what);
    format!("{what} on {place} of your library.")
}

/// The card leaves the zone it is in for its owner's library, then goes to the bottom of it when it is to.
pub(super) fn events(
    state: &GameState,
    from: ZoneKind,
    bottom: bool,
    object: ObjectId,
) -> Vec<Event> {
    let Some(found) = state.object(object) else {
        return Vec::new();
    };
    let owner = found.owner();
    let mut events = vec![Event::ZoneChanged {
        object,
        from: zone_of(from, owner),
        to: Zone::Library(owner),
        cause: ZoneCause::Moved,
    }];
    if bottom {
        events.push(Event::LibraryBottomed {
            player: owner,
            cards: vec![object],
            random: false,
        });
    }
    events
}
