//! The stack forecast of a state, worked out once however many seats see it.
//!
//! `ObjectView.forecast` is public and the same for every seat, and a batch builds a view of the state before and after
//! it for each seat. The forecast resolves the top item on a copy of the state, so it is kept here by the state's
//! fingerprint: the views of one state share one resolution, and the state after a batch is the state before the next.

use std::cell::{Cell, RefCell};

use mtg_engine::stack_forecast::{ForecastChange, stack_forecast};
use mtg_engine::state::{GameState, Stage};

/// The states remembered: the one before a batch and the one after it.
const KEPT: usize = 2;

thread_local! {
    static KNOWN: RefCell<Vec<(u64, Vec<ForecastChange>)>> = const { RefCell::new(Vec::new()) };
    static WORKED_OUT: Cell<u64> = const { Cell::new(0) };
}

/// The forecast of the top stack item of `state`; empty when there is none.
pub(crate) fn top_forecast(state: &GameState) -> Vec<ForecastChange> {
    if state.stage() != Stage::Turns || state.resolving().is_some() || state.stack().is_empty() {
        return Vec::new();
    }
    let key = state.state_hash();
    if let Some(known) = KNOWN.with_borrow(|known| {
        known
            .iter()
            .find(|(hash, _)| *hash == key)
            .map(|(_, lines)| lines.clone())
    }) {
        return known;
    }
    let lines = stack_forecast(state).unwrap_or_default();
    WORKED_OUT.set(WORKED_OUT.get().saturating_add(1));
    KNOWN.with_borrow_mut(|known| {
        if known.len() >= KEPT {
            known.remove(0);
        }
        known.push((key, lines.clone()));
    });
    lines
}

/// How many stack forecasts this thread has worked out rather than found remembered; for tests of the cost per batch.
pub fn forecasts_worked_out() -> u64 {
    WORKED_OUT.get()
}
