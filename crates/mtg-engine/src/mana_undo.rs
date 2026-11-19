//! Undoing a mana ability (CR 605, as Arena does): a source its player activated a mana ability of while holding
//! priority is untapped again, and its mana leaves the pool, while that mana is unspent and the activation affected
//! nothing else. The engine cuts the log back to before the activation, keeps the events after it and logs
//! `ManaUndone` (`docs/engine.md`, Undoing mana); this module judges which activations allow that.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::state::GameState;
use crate::take_back::{ManaRun, ManaTap};

/// Why a mana ability cannot be undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Error)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ManaUndoRefusal {
    /// Mana has left the pool since: it was spent or emptied.
    #[error("that mana was spent")]
    Spent,
    /// The activation triggered an ability.
    #[error("tapping it caused a trigger")]
    Triggered,
    /// The ability did more than tap its source and add mana: another cost, damage, a draw, a random result.
    #[error("its ability did more than add mana")]
    DidMore,
    /// Something besides more mana abilities happened since.
    #[error("something happened since")]
    Since,
}

/// A source the priority holder tapped for mana since they last did anything else, and why its activation cannot be
/// undone, if it cannot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ManaUndo {
    /// The source.
    pub source: ObjectId,
    /// Why it cannot be undone; left out when it can.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub refusal: Option<ManaUndoRefusal>,
}

/// The sources of `run`, each once (its latest activation) in the order they were activated, with what keeps each from
/// being undone; `events` is the whole log and `state` the state after it.
pub(crate) fn offers(run: &ManaRun, events: &[Event], state: &GameState) -> Vec<ManaUndo> {
    let mut found: Vec<ManaUndo> = Vec::new();
    for (index, tap) in run.taps.iter().enumerate() {
        found.retain(|undo| undo.source != tap.source);
        found.push(ManaUndo {
            source: tap.source,
            refusal: judge(run, index, events, state),
        });
    }
    found
}

/// What keeps the activation `run.taps[index]` from being undone, if anything does.
fn judge(
    run: &ManaRun,
    index: usize,
    events: &[Event],
    state: &GameState,
) -> Option<ManaUndoRefusal> {
    let Some(tap) = run.taps.get(index) else {
        return Some(ManaUndoRefusal::Since);
    };
    let own = events.get(tap.from..tap.to).unwrap_or_default();
    if let Some(refusal) = own
        .iter()
        .find_map(|event| own_refusal(event, run.player, tap))
    {
        return Some(refusal);
    }
    let since = events.get(tap.to..).unwrap_or_default();
    if let Some(refusal) = since
        .iter()
        .find_map(|event| later_refusal(event, run.player))
    {
        return Some(refusal);
    }
    (state.rng_index() != tap.rng).then_some(ManaUndoRefusal::DidMore)
}

/// The refusal one event of the activation `tap` itself makes: anything but tapping its source, adding mana to its
/// player's pool and noting the activation.
fn own_refusal(event: &Event, player: PlayerId, tap: &ManaTap) -> Option<ManaUndoRefusal> {
    match event {
        Event::ManaAbilityActivated {
            player: p, source, ..
        } if *p == player && source.id == tap.source => None,
        Event::PermanentTapped { object } if *object == tap.source => None,
        Event::ManaAdded {
            player: p, source, ..
        } if *p == player && source.is_none_or(|from| from == tap.source) => None,
        Event::PriorityGranted { player: p } if *p == player => None,
        _ if triggered(event) => Some(ManaUndoRefusal::Triggered),
        _ => Some(ManaUndoRefusal::DidMore),
    }
}

/// The refusal one event after the activation makes: anything but the player's further mana abilities and undos.
fn later_refusal(event: &Event, player: PlayerId) -> Option<ManaUndoRefusal> {
    match event {
        Event::ManaAbilityActivated { player: p, .. }
        | Event::ManaAdded { player: p, .. }
        | Event::ManaUndone { player: p, .. }
        | Event::PriorityGranted { player: p }
            if *p == player =>
        {
            None
        }
        Event::PermanentTapped { .. } => None,
        Event::ManaSpent { .. } | Event::ManaPoolEmptied { .. } => Some(ManaUndoRefusal::Spent),
        _ => Some(ManaUndoRefusal::Since),
    }
}

/// Whether `event` is an ability triggering or going on the stack.
fn triggered(event: &Event) -> bool {
    matches!(
        event,
        Event::AbilityTriggered(_)
            | Event::AbilityPutOnStack(_)
            | Event::TriggersOrdered { .. }
            | Event::DelayedTriggerCreated(_)
    )
}

/// Checks `PriorityAction::UndoMana(source)` against what is offered.
pub(crate) fn check(offered: &[ManaUndo], source: ObjectId) -> Result<(), EngineError> {
    let undo = offered
        .iter()
        .find(|undo| undo.source == source)
        .ok_or(IllegalAnswer::NotOffered("mana to undo"))?;
    match undo.refusal {
        Some(refusal) => Err(IllegalAnswer::ManaUndo(refusal).into()),
        None => Ok(()),
    }
}

/// The run once the activation `index` is undone: it is left out, and the later ones move back by its events.
pub(crate) fn without(run: &ManaRun, index: usize) -> ManaRun {
    let mut left = run.clone();
    let Some(gone) = left.taps.get(index).copied() else {
        return left;
    };
    left.taps.remove(index);
    let length = gone.to - gone.from;
    for tap in left.taps.iter_mut().skip(index) {
        tap.from = tap.from.saturating_sub(length);
        tap.to = tap.to.saturating_sub(length);
    }
    left
}

#[cfg(test)]
#[path = "mana_undo_tests.rs"]
mod tests;
