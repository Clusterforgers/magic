//! Paying energy as a cost (CR 107.14): the player removes that many energy counters from themselves.

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;

use super::Payer;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::state::GameState;

/// The kind of counter energy is.
pub(crate) fn kind() -> CounterKind {
    CounterKind::Other("energy".to_owned())
}

/// The energy `player` holds.
pub(crate) fn held(state: &GameState, player: PlayerId) -> u32 {
    state
        .player(player)
        .map_or(0, |seat| seat.counters_of(&kind()))
}

/// The energy the payer holds.
pub(super) fn most(payer: &Payer<'_>) -> u32 {
    held(payer.state, payer.player)
}

/// Whether the payer holds that much energy.
pub(super) fn payable(payer: &Payer<'_>, amount: u32) -> bool {
    most(payer) >= amount
}

/// The energy paid; paying none is no event.
pub(super) fn events(player: PlayerId, amount: u32) -> Vec<Event> {
    let Ok(paid) = i32::try_from(amount) else {
        return Vec::new();
    };
    if paid == 0 {
        return Vec::new();
    }
    vec![Event::CountersChanged(CountersChanged {
        on: Target::Player(player),
        counter: kind(),
        delta: -paid,
    })]
}
