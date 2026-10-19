//! Folding combat events: declarations, damage assignment and damage.

use super::{invalid, player_mut};
use crate::characteristics::characteristics;
use crate::combat::{Attacker, CombatState, DamageTarget};
use crate::error::EngineError;
use crate::events::Event;
use crate::keywords;
use crate::state::GameState;
use crate::zones::Zone;

fn on_battlefield(state: &GameState, id: mtg_core::ids::ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|o| o.zone() == Zone::Battlefield)
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
            source,
            target,
            amount,
        } => match target {
            DamageTarget::Player(id) => {
                let player = player_mut(state, *id)?;
                player.life = player
                    .life
                    .saturating_sub(i32::try_from(*amount).unwrap_or(i32::MAX));
            }
            DamageTarget::Object(id) => {
                // Any damage from a source with deathtouch marks a creature for destruction (CR 704.5h).
                let deathtouched = *amount > 0
                    && keywords::damage_rules(state, *source).deathtouch
                    && characteristics(state, *id).is_some_and(|c| c.is_creature());
                let perm = state
                    .objects
                    .get_mut(id)
                    .and_then(|o| o.permanent.as_mut())
                    .ok_or_else(|| invalid(format!("{id:?} cannot be dealt damage")))?;
                perm.damage = perm.damage.saturating_add(*amount);
                perm.deathtouched |= deathtouched;
            }
        },
        Event::CombatEnded => state.combat = CombatState::default(),
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
