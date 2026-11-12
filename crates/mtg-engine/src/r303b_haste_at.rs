//! "All creatures can attack your opponents and planeswalkers your opponents control as though those creatures had
//! haste" (`GameRule::AttackAsThoughHasteAt`, CR 302.6): a summoning-sick creature may attack those players and their
//! planeswalkers only.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of;
use crate::combat::{AttackTarget, defending_player};
use crate::state::GameState;

/// Whether some player a rule names, other than the creature's controller, may be attacked by it though it is sick.
pub(crate) fn r303b_attacks_sick(state: &GameState, id: ObjectId) -> bool {
    let controller = controller_of(state, id);
    state
        .rules()
        .haste_at
        .iter()
        .any(|player| Some(*player) != controller)
}

/// Keeps only the targets a sick creature may attack under the rule: the players it names and the planeswalkers they
/// control. A creature that may attack for another reason keeps them all.
pub(crate) fn r303b_sick_defenders(
    state: &GameState,
    id: ObjectId,
    defenders: &mut Vec<AttackTarget>,
) {
    if !crate::keywords::summoning_sick(state, id)
        || crate::combat_quirks::attacks_as_though_haste(state, id)
    {
        return;
    }
    let named = &state.rules().haste_at;
    defenders.retain(|target| {
        let battle = matches!(target, AttackTarget::Planeswalker(permanent)
            if crate::battle::is_battle(state, *permanent));
        !battle && defending_player(state, *target).is_some_and(|player| named.contains(&player))
    });
}

/// The players the rules in force let sick creatures attack.
pub(crate) fn r303b_named(players: &[PlayerId]) -> std::collections::BTreeSet<PlayerId> {
    players.iter().copied().collect()
}
