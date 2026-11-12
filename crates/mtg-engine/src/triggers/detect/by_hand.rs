//! By-hand lines that come due: a triggered clause the table plays by hand whose event reads (`ManualClause::when`) is
//! logged as due when that event happens, so its controller is held to apply it (`docs/engine.md`, By-hand lines).

use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use super::{Happened, causes, happenings, trigger_controller};
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// A by-hand line of a permanent whose event happened: what the `ByHandDue` event says.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ByHandDue {
    /// The permanent with the line, as it was.
    pub source: ObjectRef,
    /// Who controls it, and so applies the line.
    pub controller: PlayerId,
    /// The line as the card prints it.
    pub text: String,
}

/// The by-hand lines of the permanents that wait for `happened`, judged against `state`, in battlefield order.
fn lines_due(
    state: &GameState,
    happened: &Happened,
    caused: Option<crate::triggers::CausedBy>,
    found: &mut Vec<ByHandDue>,
) {
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        let card = object.card();
        if card.manual.iter().all(|clause| clause.when.is_none()) {
            continue;
        }
        let (source, controller) = (object.reference(), trigger_controller(state, object));
        if !state.is_alive(controller) {
            continue;
        }
        let scope = Scope::announcing(state, controller, source).with_caused(caused);
        for clause in &card.manual {
            let Some(when) = &clause.when else {
                continue;
            };
            for _ in causes(when, happened, &scope) {
                found.push(ByHandDue {
                    source,
                    controller,
                    text: clause.text.clone(),
                });
            }
        }
    }
}

/// The by-hand lines that `events` make due, in the order their events happened: each line once for each time its
/// event happens, then once for the events of the batch together. What leaves the battlefield is judged against `pre`
/// as a trigger is (CR 603.10a), the rest against `post`.
pub fn due(pre: &GameState, events: &[Event], post: &GameState) -> Vec<ByHandDue> {
    if !post.watches() {
        return Vec::new();
    }
    let caused = crate::triggers::cause_of(pre, events, post);
    let mut found = Vec::new();
    let all = happenings(pre, events, post);
    for happened in &all {
        let state = if happened.looks_back() { pre } else { post };
        lines_due(state, happened, caused, &mut found);
    }
    let (back, forward): (Vec<Happened>, Vec<Happened>) =
        all.into_iter().partition(Happened::looks_back);
    for (state, members) in [(pre, back), (post, forward)] {
        if !members.is_empty() {
            lines_due(state, &Happened::Batch(members), caused, &mut found);
        }
    }
    found.retain(|line| post.is_alive(line.controller));
    found
}
