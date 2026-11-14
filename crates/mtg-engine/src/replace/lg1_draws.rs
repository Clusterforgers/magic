//! Draws of one batch are sequential (CR 121.6, 614.6): each draw, kept or replaced, sees the libraries as the events
//! before it leave them, so a draw kept after a replaced one ("reveal until", dredge) takes the card on top then, and a
//! replacement of a later draw reveals below the cards the earlier draws took.

use mtg_core::ids::{ObjectId, PlayerId};

use super::{Item, Pass};
use crate::events::Event;
use crate::state::GameState;
use crate::zones::Zone;

impl Pass<'_> {
    /// The state as the events of the slots before `at` leave the libraries, or none when they touch no library.
    pub(super) fn lg1_libraries_before(&self, at: usize) -> Option<GameState> {
        let events: Vec<&Event> = self.slots[..at]
            .iter()
            .filter_map(|slot| match &slot.item {
                Item::One(event) if touches_library(event) => Some(event),
                _ => None,
            })
            .collect();
        if events.is_empty() {
            return None;
        }
        let mut state = self.state.clone();
        for event in events {
            lg1_step(&mut state, event);
        }
        Some(state)
    }

    /// The library of `player` as the events before `at` leave it, top first.
    pub(super) fn lg1_library_at(&self, at: usize, player: PlayerId) -> Vec<ObjectId> {
        let state = self.lg1_libraries_before(at);
        state
            .as_ref()
            .unwrap_or(self.state)
            .zone(Zone::Library(player))
            .iter()
            .rev()
            .copied()
            .collect()
    }
}

/// Names the top card in each draw as the events before it leave the library, in order.
pub(super) fn lg1_sequential_draws(state: &GameState, events: &mut [Event]) {
    let mut state = state.clone();
    for event in events.iter_mut() {
        if touches_library(event) {
            *event = lg1_step(&mut state, event);
        }
    }
}

/// Applies a library event to `state`; a draw takes the card on top now. The event as applied.
fn lg1_step(state: &mut GameState, event: &Event) -> Event {
    let event = match event {
        Event::CardDrawn { player, .. } | Event::DrewFromEmptyLibrary { player } => {
            match state.zone(Zone::Library(*player)).last() {
                Some(&object) => Event::CardDrawn {
                    player: *player,
                    object,
                },
                None => Event::DrewFromEmptyLibrary { player: *player },
            }
        }
        other => other.clone(),
    };
    // An event this batch cannot apply here leaves the library as it is; the batch's fold reports it.
    let _ = crate::fold::apply_event(state, &event);
    event
}

/// Whether the event draws from, takes from, puts into or reorders a library.
fn touches_library(event: &Event) -> bool {
    match event {
        Event::CardDrawn { .. }
        | Event::DrewFromEmptyLibrary { .. }
        | Event::LibraryShuffled { .. }
        | Event::LibraryRefilled { .. }
        | Event::LibraryBottomed { .. }
        | Event::LibraryInserted { .. } => true,
        Event::ZoneChanged { from, to, .. } => {
            matches!(from, Zone::Library(_)) || matches!(to, Zone::Library(_))
        }
        _ => false,
    }
}
