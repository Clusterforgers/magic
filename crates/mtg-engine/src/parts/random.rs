//! Discarding cards at random as a cost (CR 701.9b): the hand is shuffled with the next substream of the game's random
//! source and the first cards are discarded; the pick is logged before the discards so a replay spends the same substream.

use mtg_core::ids::PlayerId;

use super::Payer;
use crate::events::Event;
use crate::rng::GameRng;
use crate::state::GameState;
use crate::zones::Zone;

/// The cards the payer's hand holds.
pub(super) fn most(payer: &Payer<'_>) -> u32 {
    u32::try_from(payer.state.zone(Zone::Hand(payer.player)).len()).unwrap_or(u32::MAX)
}

/// Whether the hand holds that many cards.
pub(super) fn payable(payer: &Payer<'_>, amount: u32) -> bool {
    most(payer) >= amount
}

/// The pick and the discards of `amount` cards of the hand.
pub(super) fn events(state: &GameState, player: PlayerId, amount: u32) -> Vec<Event> {
    let mut cards = state.zone(Zone::Hand(player)).to_vec();
    let take = usize::try_from(amount).unwrap_or(usize::MAX);
    if cards.is_empty() || take == 0 {
        return Vec::new();
    }
    GameRng::substream(state.seed, state.rng_index).shuffle(&mut cards);
    cards.truncate(take);
    let mut events = vec![Event::ChosenAtRandom {
        cards: cards.clone(),
    }];
    events.extend(
        cards
            .into_iter()
            .map(|object| super::discard::event(player, object)),
    );
    events
}
