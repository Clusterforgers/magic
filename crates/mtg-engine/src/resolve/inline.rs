//! Effects that run in the middle of something else, without the stack: what a mana ability does besides adding mana,
//! and what a replacement effect does instead of the event it replaces.

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::prim;
use super::{Cx, ResolveError, Resolving, Run, Scope};
use crate::abilities::Cause;
use crate::events::Event;
use crate::object::{GameObject, ObjectRef};
use crate::state::GameState;

/// The events of `effects` run now for `player` as the controller of `source`, each as the effect proposes it.
///
/// An effect that needs an answer is an error: a mana ability asks nobody anything (CR 605.3b).
pub(crate) fn run_inline(
    state: &GameState,
    player: PlayerId,
    source: &GameObject,
    effects: &[Effect],
) -> Result<Vec<Event>, ResolveError> {
    let scope = Scope::announcing(state, player, source.reference());
    run_in(scope, effects)
}

/// The events of `effects` that a replacement effect of `source` runs in place of the event `cause` describes: the
/// effects read its player or object (`Triggering`), its amount and the source of its damage (`Dealer`).
///
/// An effect that needs an answer is an error: a replacement asks nothing in the middle of an event.
pub(crate) fn w5d_run_instead(
    state: &GameState,
    player: PlayerId,
    source: ObjectRef,
    cause: &Cause,
    effects: &[Effect],
) -> Result<Vec<Event>, ResolveError> {
    run_in(Scope::triggered(state, player, source, cause), effects)
}

/// Runs the effects one after the other in `scope`, refusing any that asks.
fn run_in(scope: Scope<'_>, effects: &[Effect]) -> Result<Vec<Event>, ResolveError> {
    let resolving = Resolving::begin(scope.source.id);
    let cx = Cx {
        scope,
        resolving: &resolving,
    };
    let mut events = Vec::new();
    for effect in effects {
        match prim::run(&cx, effect)? {
            Run::Done { events: made, .. } => events.extend(made),
            Run::Ask(_) | Run::Cast(_) => {
                return Err(ResolveError::Invalid(
                    "an effect run in the middle of something else asked a question".into(),
                ));
            }
        }
    }
    Ok(events)
}
