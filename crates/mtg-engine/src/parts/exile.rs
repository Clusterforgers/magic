//! Exiling cards from the payer's graveyard as a cost ("Exile a creature card from your graveyard").

use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::events::{Event, ZoneCause};
use crate::state::GameState;
use crate::zones::Zone;

/// The cards in the payer's graveyard that match.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer
            .state
            .zone(Zone::Graveyard(payer.player))
            .iter()
            .copied(),
        filter,
        |_| true,
    )
}

/// The card leaves its owner's graveyard for exile.
pub(super) fn event(state: &GameState, object: ObjectId) -> Option<Event> {
    Some(Event::ZoneChanged {
        object,
        from: Zone::Graveyard(state.object(object)?.owner()),
        to: Zone::Exile,
        cause: ZoneCause::Exiled,
    })
}

/// The cards in any player's graveyard that match.
pub(super) fn candidates_any(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    let cards: Vec<ObjectId> = payer
        .state
        .players
        .iter()
        .flat_map(|player| payer.state.zone(Zone::Graveyard(player.id)).iter().copied())
        .collect();
    payer.matching(cards.into_iter(), filter, |_| true)
}

/// "Exile a card from a graveyard."
pub(super) fn text_any(filter: &Selector, count: usize) -> String {
    text(filter, count).replace("your graveyard", "a graveyard")
}

/// "Exile a card from your graveyard.", "Exile 2 creature cards from your graveyard."
pub(super) fn text(filter: &Selector, count: usize) -> String {
    crate::wording::exile_from_graveyard(filter, count)
}

/// The permanents the payer controls and the cards in their graveyard that match: the materials of craft (CR 702.167a).
pub(super) fn craft_candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    let mine = payer
        .state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            payer
                .state
                .object(*id)
                .is_some_and(|object| object.controller() == payer.player)
        });
    let graveyard = payer
        .state
        .zone(Zone::Graveyard(payer.player))
        .iter()
        .copied();
    payer.matching(mine.chain(graveyard), filter, |_| true)
}

/// "Exile a creature from among permanents you control and/or cards in your graveyard."
pub(super) fn craft_text(filter: &Selector, count: usize) -> String {
    let (head, _) = crate::wording::phrase(filter);
    let what = if count == 1 {
        format!("one {head}")
    } else {
        format!("{count} {head}s")
    };
    format!("Exile {what} from among permanents you control and/or cards in your graveyard.")
}
