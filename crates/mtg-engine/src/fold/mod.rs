//! The single state transition: folding one [`Event`] into a [`GameState`].

mod combat;
mod opening;
mod resolution;
#[cfg(test)]
mod tests;
mod zones;

use std::collections::BTreeSet;

use mtg_core::ids::{ObjectId, PlayerId};

use crate::error::EngineError;
use crate::events::Event;
use crate::mana::ManaPool;
use crate::player::{LossReason, Player, PlayerStatus};
use crate::priority::PriorityState;
use crate::state::{GameResult, GameState, Stage};
use crate::turn::Step;

pub(crate) fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidEvent(message.into())
}

pub(crate) fn player_mut(state: &mut GameState, id: PlayerId) -> Result<&mut Player, EngineError> {
    state
        .players
        .get_mut(id.index())
        .ok_or_else(|| invalid(format!("unknown player {id:?}")))
}

fn pool_mut(state: &mut GameState, id: PlayerId) -> Result<&mut ManaPool, EngineError> {
    player_mut(state, id).map(|p| &mut p.mana)
}

/// Applies one event; the only way game state ever changes.
///
/// Returns an error (leaving the state possibly part-way changed) only for events that cannot
/// follow from the current state, which indicates an engine bug or a corrupt log.
pub fn apply_event(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::FirstPlayerChosen { .. }
        | Event::LibraryShuffled { .. }
        | Event::MulliganPhaseBegan
        | Event::HandDealt { .. }
        | Event::MulliganDeclared { .. }
        | Event::MulliganTaken { .. }
        | Event::MulliganKept { .. }
        | Event::CardsBottomed { .. }
        | Event::MulliganPhaseEnded => opening::apply(state, event),
        Event::LandPlayed { .. }
        | Event::SpellCast { .. }
        | Event::SpellResolved { .. }
        | Event::CardDrawn { .. }
        | Event::CardDiscarded { .. }
        | Event::CreatureDied { .. }
        | Event::ZoneChanged { .. }
        | Event::ObjectLeftGame { .. } => zones::apply(state, event),
        Event::AttackersDeclared { .. }
        | Event::BlockersDeclared { .. }
        | Event::DamageAssigned { .. }
        | Event::DamageDealt { .. }
        | Event::CombatEnded
        | Event::DamageCleared => combat::apply(state, event),
        Event::AbilityPutOnStack(_)
        | Event::AbilityActivated { .. }
        | Event::ResolutionBegan { .. }
        | Event::ChoiceMade { .. }
        | Event::StepDone { .. }
        | Event::ResolutionEnded { .. } => resolution::apply(state, event),
        Event::AbilityTriggered(triggered) => crate::triggers::fold_triggered(state, triggered),
        Event::TriggersOrdered { player, order } => {
            crate::triggers::fold_ordered(state, *player, order)
        }
        Event::TriggerRemoved {
            source,
            ability,
            delayed,
            cause,
        } => crate::triggers::fold_removed(state, *source, *ability, *delayed, cause),
        Event::DelayedTriggerCreated(created) => crate::triggers::fold_created(state, created),
        Event::DelayedTriggerExpired { id } => crate::triggers::fold_expired(state, *id),
        Event::CountersChanged(counters) => crate::counters::fold(state, counters),
        Event::TokenCreated(token) => crate::tokens::fold(state, token),
        // What a player learned changes nothing in the game; the protocol decides who may see it.
        Event::LibraryLooked(_) | Event::CardsFound(_) | Event::Scried(_) | Event::Revealed(_) => {
            Ok(())
        }
        _ => apply_misc(state, event),
    }
}

fn apply_misc(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::TurnBegan { number, active } => {
            state.turn.number = *number;
            state.turn.active = *active;
            state.turn.step = Step::Untap;
            state.turn.lands_played = 0;
            state.limits_spent.clear();
            state.priority = None;
            state.combat = crate::combat::CombatState::default();
            for object in state.objects.values_mut() {
                if object.controller == *active
                    && let Some(perm) = &mut object.permanent
                {
                    perm.summoning_sick = false;
                }
            }
        }
        Event::StepBegan { step } => {
            state.turn.step = *step;
            state.priority = None;
        }
        Event::NothingToDo { .. } => {}
        Event::PriorityGranted { player } => {
            state.priority = Some(PriorityState {
                holder: *player,
                passed: BTreeSet::default(),
            });
        }
        Event::PriorityPassed { player } => pass_priority(state, *player)?,
        Event::PermanentTapped { object } => set_tapped(state, *object, true)?,
        Event::PermanentUntapped { object } => set_tapped(state, *object, false)?,
        Event::ManaAdded { player, mana, .. } => {
            pool_mut(state, *player)?.add(*mana);
            reset_passes(state);
        }
        Event::ManaSpent { player, mana } => {
            let pool = pool_mut(state, *player)?;
            for m in mana {
                if !pool.spend(*m) {
                    return Err(invalid(format!(
                        "{player:?} spent {m:?} mana they do not have"
                    )));
                }
            }
        }
        Event::ManaPoolEmptied { player } => pool_mut(state, *player)?.clear(),
        Event::LifePaid { player, amount } => pay_life(state, *player, *amount)?,
        Event::LifeGained { player, amount } => change_life(state, *player, *amount, true)?,
        Event::LifeLost { player, amount } => change_life(state, *player, *amount, false)?,
        Event::DrewFromEmptyLibrary { player } => {
            player_mut(state, *player)?.drew_from_empty = true;
        }
        Event::EffectCreated { effect } => {
            if effect.layer != effect.modification.layer() {
                return Err(invalid("effect layer does not match its modification"));
            }
            if state.effects.iter().any(|e| e.id == effect.id) {
                return Err(invalid(format!("duplicate effect {:?}", effect.id)));
            }
            state.next_effect = state.next_effect.max(effect.id.0 + 1);
            state.next_timestamp = state.next_timestamp.max(effect.timestamp + 1);
            state.effects.push(effect.clone());
        }
        Event::EffectExpired { id } => {
            let before = state.effects.len();
            state.effects.retain(|e| e.id != *id);
            if state.effects.len() == before {
                return Err(invalid(format!("unknown effect {id:?}")));
            }
        }
        Event::PlayerLost { player, reason } => player_lost(state, *player, reason)?,
        Event::GameEnded { result } => end_game(state, *result),
        other => return Err(invalid(format!("unhandled event {other:?}"))),
    }
    Ok(())
}

/// Takes `amount` off a life total as a cost; a player cannot pay more life than they have (CR 119.4).
fn pay_life(state: &mut GameState, id: PlayerId, amount: u32) -> Result<(), EngineError> {
    let player = player_mut(state, id)?;
    match i32::try_from(amount) {
        Ok(paid) if paid > 0 && paid <= player.life => {
            player.life -= paid;
            Ok(())
        }
        _ => Err(invalid(format!(
            "{id:?} cannot pay {amount} life with {}",
            player.life
        ))),
    }
}

/// Adds `amount` to a life total, or takes it off; a total may go to 0 or below, which state-based actions judge.
fn change_life(
    state: &mut GameState,
    id: PlayerId,
    amount: u32,
    gain: bool,
) -> Result<(), EngineError> {
    let player = player_mut(state, id)?;
    match i32::try_from(amount) {
        Ok(change) if change > 0 => {
            player.life = if gain {
                player.life.saturating_add(change)
            } else {
                player.life.saturating_sub(change)
            };
            Ok(())
        }
        _ => Err(invalid(format!("{id:?} cannot change life by {amount}"))),
    }
}

/// Gives priority to the active player (or the next player in turn order) with nobody having passed, as it
/// is after something leaves the stack (CR 117.3b).
pub(crate) fn priority_to_active(state: &mut GameState) {
    let active = state.turn.active;
    if let Some(holder) = state.turn_order_from(active).first().copied() {
        state.priority = Some(PriorityState {
            holder,
            passed: BTreeSet::default(),
        });
    }
}

pub(crate) fn reset_passes(state: &mut GameState) {
    if let Some(priority) = &mut state.priority {
        priority.passed.clear();
    }
}

fn set_tapped(state: &mut GameState, id: ObjectId, tapped: bool) -> Result<(), EngineError> {
    let perm = state
        .objects
        .get_mut(&id)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    if perm.tapped == tapped {
        return Err(invalid(format!("{id:?} is already in that tapped state")));
    }
    perm.tapped = tapped;
    Ok(())
}

fn pass_priority(state: &mut GameState, player: PlayerId) -> Result<(), EngineError> {
    let holder = state.priority.as_ref().map(|p| p.holder);
    if holder != Some(player) {
        return Err(invalid(format!("{player:?} passed without priority")));
    }
    let order = state.turn_order_from(state.next_alive_after(player).unwrap_or(player));
    if let Some(priority) = &mut state.priority {
        priority.passed.insert(player);
        if let Some(next) = order.into_iter().find(|p| !priority.passed.contains(p)) {
            priority.holder = next;
        }
    }
    Ok(())
}

fn player_lost(
    state: &mut GameState,
    id: PlayerId,
    reason: &LossReason,
) -> Result<(), EngineError> {
    let player = player_mut(state, id)?;
    if !player.is_in_game() {
        return Err(invalid(format!("{id:?} already lost")));
    }
    player.status = PlayerStatus::Lost(reason.clone());
    player.drew_from_empty = false;
    state
        .combat
        .remove_attackers_targeting(crate::combat::AttackTarget::Player(id));
    let successor = state.next_alive_after(id);
    if let Some(priority) = &mut state.priority {
        priority.passed.remove(&id);
        if priority.holder == id
            && let Some(next) = successor
        {
            priority.holder = next;
        }
    }
    Ok(())
}

fn end_game(state: &mut GameState, result: GameResult) {
    state.stage = Stage::Finished;
    state.result = Some(result);
    state.priority = None;
}
