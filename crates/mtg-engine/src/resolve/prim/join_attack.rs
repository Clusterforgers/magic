//! `Effect::JoinAttack`: a creature put onto the battlefield joins the attack of the creature the cost returned (CR 702.49a).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::characteristics::characteristics;
use crate::combat::AttackTarget;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::state::GameState;
use crate::zones::Zone;

/// Whether the player or planeswalker is still there to be attacked.
fn attackable(state: &GameState, target: AttackTarget) -> bool {
    match target {
        AttackTarget::Player(player) => state.player(player).is_some(),
        AttackTarget::Planeswalker(id) => state
            .object(id)
            .is_some_and(|object| object.zone() == Zone::Battlefield),
    }
}

/// Each creature `what` names that is still the object named, on the battlefield under the ability's controller, enters
/// the attack tapped and attacking what the object picked for the cost was attacking as it left combat. Nothing joins when
/// no cost pick left an attack or what it attacked is gone; the creature is never declared as an attacker, so nothing
/// that triggers on attacking sees it (CR 508.4).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::JoinAttack { what } = effect else {
        return Err(wrong_effect("JoinAttack"));
    };
    let state = cx.state();
    let combat = state.combat();
    let target = cx
        .scope
        .choices
        .cost_picks()
        .iter()
        .flatten()
        .find_map(|picked| combat.left_attacking(*picked))
        .filter(|target| combat.attackers_declared() && attackable(state, *target));
    let Some(target) = target else {
        return Ok(Run::events(Vec::new()));
    };
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| {
            object.zone() == Zone::Battlefield
                && object.controller() == cx.scope.controller
                && characteristics(state, object.id()).is_some_and(|c| c.is_creature())
                && !combat.attackers().iter().any(|a| a.object() == object.id())
        })
        .map(|object| Event::JoinedAttack {
            object: object.id(),
            target,
        })
        .collect();
    Ok(Run::events(events))
}
