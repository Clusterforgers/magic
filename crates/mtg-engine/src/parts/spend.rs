//! Spending counters as a cost ("Spend 3 ectoplasm"): the player removes that many counters of the kind from themselves,
//! and cannot pay with fewer.

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;

use super::Payer;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;

/// The counters of the kind the payer holds.
pub(super) fn most(payer: &Payer<'_>, counter: &CounterKind) -> u32 {
    payer
        .state
        .player(payer.player)
        .map_or(0, |seat| seat.counters_of(counter))
}

/// Whether the payer holds that many counters of the kind.
pub(super) fn payable(payer: &Payer<'_>, counter: &CounterKind, count: u32) -> bool {
    most(payer, counter) >= count
}

/// The counters spent; spending none is no event.
pub(super) fn events(player: PlayerId, counter: &CounterKind, count: u32) -> Vec<Event> {
    let Ok(spent) = i32::try_from(count) else {
        return Vec::new();
    };
    if spent == 0 {
        return Vec::new();
    }
    vec![Event::CountersChanged(CountersChanged {
        on: Target::Player(player),
        counter: counter.clone(),
        delta: -spent,
    })]
}

/// The part in the words of a card: "Spend 3 ectoplasm".
pub(super) fn text(counter: &CounterKind, count: u32) -> String {
    format!("Spend {count} {}", counter.name())
}
