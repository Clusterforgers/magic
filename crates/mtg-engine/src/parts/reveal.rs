//! Revealing cards from the payer's hand as a cost ("Reveal a Giant card from your hand"): they stay in the hand.

use mtg_cards::cost::CostItem;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::value::Amount;
use mtg_core::ids::{ObjectId, PlayerId};

use super::Payer;
use crate::events::Event;
use crate::library::Revealed;
use crate::state::GameState;
use crate::zones::Zone;

/// The cards in the payer's hand that match.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer.state.zone(Zone::Hand(payer.player)).iter().copied(),
        filter,
        |_| true,
    )
}

/// The card is shown to every player; it does not move.
pub(super) fn event(player: PlayerId, object: ObjectId) -> Event {
    Event::Revealed(Revealed {
        player,
        cards: vec![object],
    })
}

/// "Reveal a Giant card from your hand.", "Reveal 2 cards from your hand."
pub(super) fn text(filter: &Selector, count: usize) -> String {
    crate::wording::reveal_from_hand(filter, count)
}

/// "Reveal your hand": a reveal from the hand counted as every card of the payer's hand, with no filter.
pub(super) fn reveals_the_hand(item: &CostItem) -> bool {
    matches!(item, CostItem::RevealFromHand { count: Amount::Count(cards), filter }
        if filter.is_empty() && cards.0 == [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)])
}

/// Every card of the payer's hand but the spell being cast (on the stack as its costs are paid, CR 601.2a) is shown to
/// every player at once; an empty hand shows nothing.
pub(super) fn hand_events(state: &GameState, player: PlayerId, source: ObjectId) -> Vec<Event> {
    let cards: Vec<ObjectId> = state
        .zone(Zone::Hand(player))
        .iter()
        .copied()
        .filter(|card| *card != source)
        .collect();
    if cards.is_empty() {
        return Vec::new();
    }
    vec![Event::Revealed(Revealed { player, cards })]
}
