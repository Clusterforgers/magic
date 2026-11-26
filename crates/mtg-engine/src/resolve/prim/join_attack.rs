//! `Effect::JoinAttack`: a creature put onto the battlefield joins the attack of the creature the cost returned (CR 702.49a).

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;

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
        AttackTarget::Player(player) => {
            state.player(player).is_some() && crate::spirits::attackable(state, player)
        }
        AttackTarget::Planeswalker(id) => state
            .object(id)
            .is_some_and(|object| object.zone() == Zone::Battlefield),
    }
}

/// What the creatures attack: the player `against` names, or what the object picked for the cost was attacking as it left combat (ninjutsu), or, with no
/// pick for the cost, what their controller chooses among the defenders of their own attack (CR 508.4, asked only when there
/// is more than one). Nothing when no attack is under way or what the pick attacked is gone.
fn aim(
    cx: &Cx<'_>,
    against: Option<&Ref>,
) -> Result<Result<Option<AttackTarget>, Decision>, ResolveError> {
    let state = cx.state();
    let combat = state.combat();
    if let Some(against) = against {
        // The player the effect names, while an attack is under way and that player can still be attacked.
        return Ok(Ok(cx
            .scope
            .players(against)?
            .into_iter()
            .map(AttackTarget::Player)
            .find(|target| {
                combat.attackers_declared() && attackable(state, *target)
            })));
    }
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
    let Effect::JoinAttack { what, against } = effect else {
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
    let target = match aim(cx, against.as_ref())? {
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

/// What the creatures `controller` controls are attacking now, each once in the order of the attack: only a player or
/// planeswalker a creature still attacks counts, and never a planeswalker that has left (CR 506.4c).
fn r5084_attacked(state: &GameState, controller: mtg_core::ids::PlayerId) -> Vec<AttackTarget> {
    let mut attacked = Vec::new();
    for attacker in state.combat().attackers() {
        let theirs = state.object(attacker.object()).is_some_and(|object| {
            crate::characteristics::controller_of_object(state, object) == controller
        });
        if theirs
            && !attacker.attacks_nothing()
            && attackable(state, attacker.target())
            && !attacked.contains(&attacker.target())
        {
            attacked.push(attacker.target());
        }
    }
    attacked
}

/// The words of a player or planeswalker to attack.
fn r5084_option(state: &GameState, target: AttackTarget) -> crate::choose::ChoiceOption {
    match target {
        AttackTarget::Player(seat) => crate::choose::ChoiceOption::Player(seat),
        AttackTarget::Planeswalker(walker) => crate::choose::ChoiceOption::Label(
            state
                .object(walker)
                .map_or_else(String::new, |o| o.card().name.clone()),
        ),
    }
}

/// `Effect::R5084JoinTheirAttack`: the first creature `what` names that is on the battlefield and not attacking yet
/// joins the attack of the active player when that player controls it now; the effect's controller chooses which of
/// the players and planeswalkers that player's creatures are attacking it attacks (asked only when there is more than
/// one). With none, it does not attack.
pub(super) fn r5084_run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R5084JoinTheirAttack { what } = effect else {
        return Err(wrong_effect("R5084JoinTheirAttack"));
    };
    let state = cx.state();
    let combat = state.combat();
    if !combat.attackers_declared() {
        return Ok(Run::events(Vec::new()));
    }
    let Some(object) = cx.scope.live_objects(what)?.into_iter().find(|object| {
        object.zone() == Zone::Battlefield
            && characteristics(state, object.id()).is_some_and(|c| c.is_creature())
            && !combat.attackers().iter().any(|a| a.object() == object.id())
            && state.is_active(crate::characteristics::controller_of_object(state, object))
    }) else {
        return Ok(Run::events(Vec::new()));
    };
    let targets = r5084_attacked(
        state,
        crate::characteristics::controller_of_object(state, object),
    );
    let target = match targets.as_slice() {
        [] => return Ok(Run::events(Vec::new())),
        [only] => *only,
        _ => {
            let chooser = OptionChooser {
                player: cx.scope.controller,
                options: targets
                    .iter()
                    .map(|target| r5084_option(state, *target))
                    .collect(),
                text: "Choose a player or planeswalker for it to attack.".into(),
            };
            match pick_options(cx, &[chooser])? {
                Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
                Optioned::Done(taken) => {
                    match taken.first().and_then(|(_, index)| targets.get(*index)) {
                        Some(target) => *target,
                        None => return Ok(Run::events(Vec::new())),
                    }
                }
            }
        }
    };
    Ok(Run::events(vec![Event::JoinedAttack {
        object: object.id(),
        target,
    }]))
}
