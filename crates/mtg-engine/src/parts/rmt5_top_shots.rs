//! The cards an "exile the top card of your library" cost takes, as they were before it was paid, so the effects can
//! name them (`Ref::Paid`: "where X is the exiled card's mana value", "if the exiled card is a snow land").

use mtg_core::ids::PlayerId;

use super::Part;
use crate::library::top_cards;
use crate::snapshot::{Snapshot, snapshot};
use crate::state::GameState;

/// The top cards of the payer's library each `ExileTop` part of the cost will exile, the top card first.
pub(crate) fn rmt5_top_shots(state: &GameState, player: PlayerId, parts: &[Part]) -> Vec<Snapshot> {
    let wanted: usize = parts
        .iter()
        .filter_map(|part| match part {
            Part::ExileTop(count) => usize::try_from(*count).ok(),
            _ => None,
        })
        .sum();
    top_cards(state, player, wanted)
        .into_iter()
        .filter_map(|id| snapshot(state, id))
        .collect()
}
