//! "This creature can attack players who attacked you during their last turn as though it didn't have defender."
//! (`CombatQuirk::AttackLastTurnAttackersDespiteDefender`): a creature with defender may attack, and only those players.

use mtg_cards::modify::CombatQuirk;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of;
use crate::combat::AttackTarget;
use crate::state::GameState;

/// Whether the creature holds the quirk.
fn r5523_has_quirk(state: &GameState, id: ObjectId) -> bool {
    state.rules().quirks.has(id, |q| {
        matches!(q, CombatQuirk::AttackLastTurnAttackersDespiteDefender)
    })
}

/// The players the creature may attack under the quirk: those who attacked its controller during their last turn.
fn r5523_named(state: &GameState, id: ObjectId) -> Vec<PlayerId> {
    let Some(controller) = controller_of(state, id) else {
        return Vec::new();
    };
    state
        .alive_players()
        .filter(|player| *player != controller)
        .filter(|player| state.r305b_attacked_last_turn(*player, controller))
        .collect()
}

/// Whether a creature that defender keeps from attacking may still attack someone under the quirk.
pub(crate) fn r5523_attacks_some_despite_defender(state: &GameState, id: ObjectId) -> bool {
    r5523_has_quirk(state, id) && !r5523_named(state, id).is_empty()
}

/// Keeps only the players the quirk lets a creature with defender attack; a creature that may attack for another reason
/// (no defender, or "as though it didn't have defender") keeps every target.
pub(crate) fn r5523_defender_targets(
    state: &GameState,
    id: ObjectId,
    defenders: &mut Vec<AttackTarget>,
) {
    if !crate::keywords::forbids_attacking(state, id)
        || crate::combat_quirks::attacks_despite_defender(state, id)
        || !r5523_has_quirk(state, id)
    {
        return;
    }
    let named = r5523_named(state, id);
    defenders
        .retain(|target| matches!(target, AttackTarget::Player(player) if named.contains(player)));
}
