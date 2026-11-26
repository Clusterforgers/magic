//! "Counter target spell if it would destroy a land you control" (`Condition::R7018WouldDestroy`, the Equinox rulings):
//! the spell is read as it would resolve right now, with its controller and its targets, and the condition holds when
//! one of its own `Destroy` steps, at the top of its text or in a batch, names a permanent the judging scope's selector
//! picks. A destruction behind a choice ("may", "unless"), a mode, a condition, a chance or a game rule (lethal damage)
//! is not one the spell would do.

use mtg_cards::effect::Effect;
use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;

use super::{Resolving, Scope, rules_of, scope_of};

/// The `Destroy` steps of `effects` the spell does whatever happens: those at the top and inside its batches.
fn destroys<'a>(effects: &'a [Effect], found: &mut Vec<&'a mtg_cards::refs::Ref>) {
    for effect in effects {
        match effect {
            Effect::Destroy { what, .. } => found.push(what),
            Effect::Together { effects } => destroys(effects, found),
            _ => {}
        }
    }
}

/// Whether the spell `spell` would destroy a permanent `what` picks, judged by `judge` (whose "you" is the player who
/// asks), if it resolved now.
pub(super) fn r7018_would_destroy(judge: &Scope<'_>, spell: ObjectId, what: &Selector) -> bool {
    let state = judge.state;
    let Some(object) = state
        .object(spell)
        .filter(|object| object.ability().is_none())
    else {
        return false;
    };
    let Some(rules) = rules_of(state, object) else {
        return false;
    };
    let resolving = Resolving::begin(spell);
    let scope = scope_of(state, object, &resolving, &rules);
    let mut found = Vec::new();
    destroys(rules.effects(), &mut found);
    found.into_iter().any(|named| {
        scope.live_objects(named).is_ok_and(|objects| {
            objects
                .iter()
                .any(|object| judge.selects(what, object.reference()).unwrap_or(false))
        })
    })
}
