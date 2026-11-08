//! `Effect::JoinAttack`: a creature put onto the battlefield joins the attack of the creature the cost returned (CR 702.49a).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::characteristics::characteristics;
use crate::choose::{OptionChooser, Optioned, pick_options};
use crate::combat::AttackTarget;
use crate::decision::Decision;
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

/// What the creatures attack: what the object picked for the cost was attacking as it left combat (ninjutsu), or, with no
/// pick for the cost, what their controller chooses among the defenders of their own attack (CR 508.4, asked only when there
/// is more than one). Nothing when no attack is under way or what the pick attacked is gone.
fn aim(cx: &Cx<'_>) -> Result<Result<Option<AttackTarget>, Decision>, ResolveError> {
    let state = cx.state();
    let combat = state.combat();
    let mut picks = cx.scope.choices.cost_picks().iter().flatten().peekable();
    if picks.peek().is_some() {
        return Ok(Ok(picks
            .find_map(|picked| combat.left_attacking(*picked))
            .filter(|target| {
                combat.attackers_declared() && attackable(state, *target)
            })));
    }
    let player = cx.scope.controller;
    let targets = super::create_token::attackable(state, player);
    if targets.is_empty() {
        return Ok(Ok(None));
    }
    let chooser = OptionChooser {
        player,
        options: targets.iter().map(|(_, option)| option.clone()).collect(),
        text: "Choose a player or planeswalker for it to attack.".into(),
    };
    Ok(match pick_options(cx, &[chooser])? {
        Optioned::Ask(decision) => Err(decision),
        Optioned::Done(taken) => Ok(taken
            .first()
            .and_then(|(_, index)| targets.get(*index))
            .map(|(target, _)| *target)),
    })
}

/// Each creature `what` names that is still the object named, on the battlefield under the ability's controller, enters
/// the attack as what `aim` says; the creature is never declared as an attacker, so nothing that triggers on attacking sees
/// it (CR 508.4). Nobody joins, nobody is asked.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::JoinAttack { what } = effect else {
        return Err(wrong_effect("JoinAttack"));
    };
    let state = cx.state();
    let combat = state.combat();
    let joining: Vec<_> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| {
            object.zone() == Zone::Battlefield
                && object.controller() == cx.scope.controller
                && characteristics(state, object.id()).is_some_and(|c| c.is_creature())
                && !combat.attackers().iter().any(|a| a.object() == object.id())
        })
        .collect();
    if joining.is_empty() {
        return Ok(Run::events(Vec::new()));
    }
    let target = match aim(cx)? {
        Err(decision) => return Ok(Run::Ask(decision)),
        Ok(None) => return Ok(Run::events(Vec::new())),
        Ok(Some(target)) => target,
    };
    let events = joining
        .into_iter()
        .map(|object| Event::JoinedAttack {
            object: object.id(),
            target,
        })
        .collect();
    Ok(Run::events(events))
}
