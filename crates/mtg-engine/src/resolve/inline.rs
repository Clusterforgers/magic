//! Effects that run in the middle of something else, without the stack: what a mana ability does besides adding mana.

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::prim;
use super::{Cx, ResolveError, Resolving, Run, Scope};
use crate::events::Event;
use crate::object::GameObject;
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
    let resolving = Resolving::begin(source.id());
    let cx = Cx {
        scope: Scope::announcing(state, player, source.reference()),
        resolving: &resolving,
    };
    let mut events = Vec::new();
    for effect in effects {
        match prim::run(&cx, effect)? {
            Run::Done { events: made, .. } => events.extend(made),
            Run::Ask(_) | Run::Cast(_) => {
                return Err(ResolveError::Invalid(
                    "an effect of a mana ability asked a question".into(),
                ));
            }
        }
    }
    Ok(events)
}
