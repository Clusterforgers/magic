//! Folding combat events: declarations, damage assignment and damage.

use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;

use super::{invalid, player_mut};
use crate::characteristics::characteristics;
use crate::choose::Target;
use crate::combat::{Attacker, CombatState, DamageForm, DamageTarget};
use crate::counters::{CountersChanged, change};
use crate::error::EngineError;
use crate::events::Event;
use crate::object::GameObject;
use crate::state::{CombatHit, GameState, Tally};
use crate::zones::Zone;

fn on_battlefield(state: &GameState, id: mtg_core::ids::ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|o| o.zone() == Zone::Battlefield)
}

/// A creature that was not declared as an attacker joins the attack, unblocked (CR 508.4).
fn join_attack(
    state: &mut GameState,
    object: mtg_core::ids::ObjectId,
    target: crate::combat::AttackTarget,
) -> Result<(), EngineError> {
    if !on_battlefield(state, object) || !state.combat.attackers_declared {
        return Err(invalid(
            "a creature joined an attack that was not under way",
        ));
    }
    state.combat.attackers.push(Attacker {
        object,
        target,
        blocked: false,
        blockers: Vec::new(),
        assignment: None,
        target_left: false,
    });
    Ok(())
}

/// Damage to a permanent: marked on a creature (CR 120.3e), taken off a planeswalker as that many loyalty counters
/// (CR 120.3c), both for a planeswalker that is a creature too. A planeswalker is told by the card it prints, so a creature
/// that takes damage never asks the layers.
fn damage_object(
    state: &mut GameState,
    id: mtg_core::ids::ObjectId,
    amount: u32,
    deathtouch: bool,
    form: DamageForm,
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
    // Wither and infect: the damage to a creature is -1/-1 counters, not damage marked (CR 120.3d).
    let counters = creature && form.makes_counters();
    if creature {
        if !counters {
            perm.damage = perm.damage.saturating_add(amount);
        }
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
    if counters && amount > 0 {
        let on = state
            .object(id)
            .map(GameObject::reference)
            .ok_or_else(|| invalid(format!("{id:?} cannot be dealt damage")))?;
        crate::counters::fold(
            state,
            &CountersChanged {
                on: Target::Object(on),
                counter: CounterKind::MinusOneMinusOne,
                delta: i32::try_from(amount)
                    .map_err(|_| invalid("too much damage dealt as counters"))?,
            },
        )?;
    }
    if amount > 0 {
        tally(state, id, Tally::Damaged);
    }
    Ok(())
}

/// Damage to a player: life lost, or with infect poison counters instead (CR 120.3a, 120.3b), and the poison counters of toxic
/// (CR 702.164) on top; combat damage a commander deals counts toward commander damage (CR 903.10a).
fn damage_player(
    state: &mut GameState,
    source: mtg_core::ids::ObjectId,
    id: mtg_core::ids::PlayerId,
    (amount, form, toxic): (u32, DamageForm, u32),
) -> Result<(), EngineError> {
    let in_combat = crate::replace::in_combat_damage(state);
    let combat = state.commanders.any() && in_combat;
    if in_combat && amount > 0 {
        record_hit(state, source);
    }
    let player = player_mut(state, id)?;
    if !matches!(form, DamageForm::Infect | DamageForm::NoLifeLoss) {
        player.life = player
            .life
            .saturating_sub(i32::try_from(amount).unwrap_or(i32::MAX));
    }
    let poison = match form {
        DamageForm::Infect => amount.saturating_add(toxic),
        _ => toxic,
    };
    if combat {
        state.commanders.dealt(source, id, amount);
    }
    if poison > 0 {
        crate::counters::fold(
            state,
            &CountersChanged {
                on: Target::Player(id),
                counter: CounterKind::Poison,
                delta: i32::try_from(poison).map_err(|_| invalid("too many poison counters"))?,
            },
        )?;
    }
    Ok(())
}

/// Notes that the creature `source` dealt combat damage to a player, with its controller and creature types as they are now
/// (prowl, CR 702.76a).
fn record_hit(state: &mut GameState, source: mtg_core::ids::ObjectId) {
    let Some(controller) = crate::characteristics::controller_of(state, source) else {
        return;
    };
    let (types, every_type) = crate::characteristics::creature_types(state, source);
    state.tallies.hits.push(CombatHit {
        controller,
        types,
        every_type,
    });
}

/// Counts one more event of the kind for the object `id` (as it is now) this turn.
fn tally(state: &mut GameState, id: mtg_core::ids::ObjectId, kind: Tally) {
    if let Some(reference) = state.object(id).map(GameObject::reference) {
        state.tally_object(reference, None, kind);
    }
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
            for (attacker, _) in attackers {
                tally(state, *attacker, Tally::Attacked);
            }
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
            deathtouch,
            form,
            toxic,
        } => match target {
            DamageTarget::Player(id) => {
                damage_player(state, *source, *id, (*amount, *form, *toxic))?;
            }
            DamageTarget::Object(id) => damage_object(state, *id, *amount, *deathtouch, *form)?,
        },
        Event::CombatEnded => state.combat = CombatState::default(),
        Event::JoinedAttack { object, target } => join_attack(state, *object, *target)?,
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
