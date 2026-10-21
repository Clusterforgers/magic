//! Folding combat events: declarations, damage assignment and damage.

use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;

use super::{invalid, player_mut};
use crate::characteristics::characteristics;
use crate::combat::{Attacker, CombatState, DamageTarget};
use crate::counters::change;
use crate::error::EngineError;
use crate::events::Event;
use crate::state::GameState;
use crate::zones::Zone;

fn on_battlefield(state: &GameState, id: mtg_core::ids::ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|o| o.zone() == Zone::Battlefield)
}

/// Damage to a permanent: marked on a creature (CR 120.3e), taken off a planeswalker as that many loyalty counters
/// (CR 120.3c), both for a planeswalker that is a creature too. A planeswalker is told by the card it prints, so a creature
/// that takes damage never asks the layers.
fn damage_object(
    state: &mut GameState,
    id: mtg_core::ids::ObjectId,
    amount: u32,
    deathtouch: bool,
) -> Result<(), EngineError> {
    let walker = state
        .object(id)
        .is_some_and(|o| o.base_types().contains(CardType::Planeswalker));
    let creature = !walker || characteristics(state, id).is_some_and(|c| c.is_creature());
    // Any damage from a source with deathtouch marks a creature for destruction (CR 704.5h).
    let deathtouched =
        amount > 0 && deathtouch && characteristics(state, id).is_some_and(|c| c.is_creature());
    let perm = state
        .objects
        .get_mut(&id)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{id:?} cannot be dealt damage")))?;
    if creature {
        perm.damage = perm.damage.saturating_add(amount);
        perm.deathtouched |= deathtouched;
    }
    if walker {
        // The damage dealt is all of it; the counters that go are as many as there are.
        let removed = amount.min(perm.counters_of(&CounterKind::Loyalty));
        if removed > 0 {
            let delta = -i32::try_from(removed)
                .map_err(|_| invalid("too many loyalty counters removed"))?;
            change(&mut perm.counters, &CounterKind::Loyalty, delta)?;
        }
    }
    Ok(())
}

pub(super) fn apply(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::AttackersDeclared { attackers, .. } => {
            if state.combat.attackers_declared {
                return Err(invalid("attackers were already declared"));
            }
            if let Some((bad, _)) = attackers.iter().find(|(o, _)| !on_battlefield(state, *o)) {
                return Err(invalid(format!(
                    "attacker {bad:?} is not on the battlefield"
                )));
            }
            state.combat.attackers = attackers
                .iter()
                .map(|(object, target)| Attacker {
                    object: *object,
                    target: *target,
                    blocked: false,
                    blockers: Vec::new(),
                    assignment: None,
                    target_left: false,
                })
                .collect();
            state.combat.attackers_declared = true;
        }
        Event::BlockersDeclared { player, blocks } => {
            if state.combat.blockers_declared.contains(player) {
                return Err(invalid(format!("{player:?} already declared blockers")));
            }
            for (blocker, attacker) in blocks {
                if !on_battlefield(state, *blocker) {
                    return Err(invalid(format!(
                        "blocker {blocker:?} is not on the battlefield"
                    )));
                }
                let target = state
                    .combat
                    .attackers
                    .iter_mut()
                    .find(|a| a.object == *attacker)
                    .ok_or_else(|| invalid(format!("{attacker:?} is not attacking")))?;
                target.blockers.push(*blocker);
                target.blocked = true;
            }
            state.combat.blockers_declared.push(*player);
        }
        Event::DamageAssigned {
            attacker,
            assignment,
        } => {
            let target = state
                .combat
                .attackers
                .iter_mut()
                .find(|a| a.object == *attacker)
                .ok_or_else(|| invalid(format!("{attacker:?} is not attacking")))?;
            target.assignment = Some(assignment.clone());
        }
        Event::DamageDealt {
            target,
            amount,
            deathtouch,
            ..
        } => match target {
            DamageTarget::Player(id) => {
                let player = player_mut(state, *id)?;
                player.life = player
                    .life
                    .saturating_sub(i32::try_from(*amount).unwrap_or(i32::MAX));
            }
            DamageTarget::Object(id) => damage_object(state, *id, *amount, *deathtouch)?,
        },
        Event::CombatEnded => state.combat = CombatState::default(),
        Event::Regenerated { object } => {
            let perm = state
                .objects
                .get_mut(object)
                .and_then(|o| o.permanent.as_mut())
                .ok_or_else(|| invalid(format!("{object:?} is not a permanent")))?;
            perm.tapped = true;
            perm.damage = 0;
            perm.deathtouched = false;
            state.combat.remove_object(*object);
        }
        Event::DamageCleared => {
            for object in state.objects.values_mut() {
                if let Some(perm) = &mut object.permanent {
                    perm.damage = 0;
                    perm.deathtouched = false;
                }
            }
        }
        other => return Err(invalid(format!("not a combat event: {other:?}"))),
    }
    Ok(())
}
