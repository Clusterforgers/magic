//! Removing a counter from a permanent the payer picks as a cost ("Remove a +1/+1 counter from a creature you control").
//!
//! The pick's filter holds the kind as `HasCounter`, so only a permanent with such a counter is offered, and each
//! permanent picked loses one counter of that kind.

use mtg_cards::refs::{Pred, Selector};
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::characteristics::controller_of;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::state::GameState;
use crate::zones::Zone;

/// The kind of counter the pick removes: the `HasCounter` of its filter.
pub(crate) fn c11c_kind(filter: &Selector) -> Option<&CounterKind> {
    filter.0.iter().find_map(|pred| match pred {
        Pred::HasCounter(kind) => Some(kind),
        _ => None,
    })
}

/// The permanents the payer controls that match, a counter of the kind among them.
pub(super) fn c11c_candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer.state.zone(Zone::Battlefield).iter().copied(),
        filter,
        |id| controller_of(payer.state, id) == Some(payer.player),
    )
}

/// One counter of the kind leaves the permanent picked.
pub(super) fn c11c_event(state: &GameState, filter: &Selector, object: ObjectId) -> Option<Event> {
    Some(Event::CountersChanged(CountersChanged {
        on: Target::Object(state.object(object)?.reference()),
        counter: c11c_kind(filter)?.clone(),
        delta: -1,
    }))
}

/// "Remove a +1/+1 counter from a creature you control", in the words of the card.
pub(super) fn c11c_text(filter: &Selector, count: usize) -> String {
    let kind = c11c_kind(filter).map_or("", CounterKind::name);
    let rest = Selector(
        filter
            .0
            .iter()
            .filter(|pred| !matches!(pred, Pred::HasCounter(_)))
            .cloned()
            .collect(),
    );
    let whom = crate::wording::sacrifice(&rest, count);
    let whom = whom.trim_start_matches("Sacrifice ").trim_end_matches('.');
    if count == 1 {
        format!("Remove a {kind} counter from {whom} you control")
    } else {
        format!("Remove a {kind} counter from each of {whom} you control")
    }
}
