//! "All Walls able to block ~ do so" (`CombatQuirk::MustBeBlockedBy`, CR 509.1c): a requirement on the block
//! declaration that binds only the blockers the selector picks.

use mtg_cards::modify::CombatQuirk;
use mtg_core::ids::ObjectId;

use crate::resolve::Scope;
use crate::state::GameState;

/// Whether `blocker` must block `attacker` if able: a rule says every creature its selector picks, judged for the
/// attacker's controller, blocks the attacker.
pub(crate) fn r303b_must_block(state: &GameState, attacker: ObjectId, blocker: ObjectId) -> bool {
    let (Some(held), Some(blocking)) = (state.object(attacker), state.object(blocker)) else {
        return false;
    };
    let controller = crate::characteristics::controller_of_object(state, held);
    let scope = Scope::announcing(state, controller, held.reference());
    state.rules().quirks.has(attacker, |quirk| match quirk {
        CombatQuirk::MustBeBlockedBy(blockers) => scope
            .selects(blockers, blocking.reference())
            .unwrap_or(false),
        _ => false,
    })
}
