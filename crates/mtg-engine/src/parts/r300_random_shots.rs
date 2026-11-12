//! The cards a "discard a card at random" cost takes, as they were before it was paid, so the effects can name them
//! (`Ref::Paid`: "equal to the mana value of the discarded card", "if the discarded card was multicolored").

use mtg_core::ids::PlayerId;

use super::Part;
use crate::snapshot::{Snapshot, snapshot};
use crate::state::GameState;

/// The cards of the payer's hand each `DiscardRandom` part of the cost will discard, picked as the payment picks them.
pub(crate) fn r300_random_shots(
    state: &GameState,
    player: PlayerId,
    parts: &[Part],
) -> Vec<Snapshot> {
    let wanted: u32 = parts
        .iter()
        .filter_map(|part| match part {
            Part::DiscardRandom(count) => Some(*count),
            _ => None,
        })
        .sum();
    super::random::picked(state, player, wanted)
        .into_iter()
        .filter_map(|id| snapshot(state, id))
        .collect()
}
