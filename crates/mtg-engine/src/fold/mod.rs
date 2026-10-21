//! The single state transition: folding one [`Event`] into a [`GameState`].

mod combat;
mod opening;
mod resolution;
#[cfg(test)]
mod tests;
mod zones;

use std::collections::BTreeSet;

use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of;
use crate::effects::Modification;
use crate::error::EngineError;
use crate::events::Event;
use crate::mana::ManaPool;
use crate::player::{LossReason, Player, PlayerStatus};
use crate::priority::PriorityState;
use crate::sight;
use crate::state::{GameResult, GameState, Stage};
use crate::triggers::{has_state_trigger, triggers_something};
use crate::turn::Step;
use crate::zones::Zone;

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
    // What the layers make of the permanents is read from the state, which this event changes.
    // The rules in force read the same things, so they go with the layers.
    let changes_layers = !crate::layers::leaves_alone(event);
    if changes_layers {
        state.layers.reset();
        state.rules_cache.reset();
    }
    let folded = fold_event(state, event);
    // A read in the middle of the fold saw the state before the rest of the event: forget it.
    if changes_layers {
        state.layers.reset_if_built();
        state.rules_cache.reset_if_built();
    }
    folded
}

fn fold_event(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::FirstPlayerChosen { .. }
        | Event::LibraryShuffled { .. }
        | Event::LibraryRefilled { .. }
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
        | Event::Entered { .. }
        | Event::ObjectLeftGame { .. } => zones::apply(state, event),
        Event::AttackersDeclared { .. }
        | Event::BlockersDeclared { .. }
        | Event::DamageAssigned { .. }
        | Event::DamageDealt { .. }
        | Event::CombatEnded
        | Event::Regenerated { .. }
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
            origin,
            delayed,
            cause,
        } => crate::triggers::fold_removed(state, *source, *origin, *ability, *delayed, cause),
        Event::DelayedTriggerCreated(created) => crate::triggers::fold_created(state, created),
        Event::DelayedTriggerExpired { id } => crate::triggers::fold_expired(state, *id),
        Event::RuleCreated { effect } => crate::rules::fold_created(state, effect),
        Event::RuleExpired { id } => crate::rules::fold_expired(state, *id),
        Event::ShieldUsed { id, left } => crate::rules::fold_used(state, *id, *left),
        Event::CountersChanged(counters) => crate::counters::fold(state, counters),
        Event::Attached { object, to } => crate::attach::fold_attached(state, *object, *to),
        Event::Unattached { object } => crate::attach::fold_unattached(state, *object),
        Event::TokenCreated(token) => crate::tokens::fold(state, token),
        Event::Scried(scried) => {
            zones::scry(state, scried)?;
            opening::scried(state, scried.player)?;
            let seen: Vec<ObjectId> = scried.top.iter().chain(&scried.bottom).copied().collect();
            sight::show(state, Some(scried.player), &seen);
            Ok(())
        }
        Event::LibraryBottomed {
            player,
            cards,
            random,
        } => {
            zones::bottom(state, *player, cards)?;
            // A random order spent a substream of the generator, as a shuffle does.
            state.rng_index += u64::from(*random);
            Ok(())
        }
        // What a player learned changes nothing in the game but who may identify the cards until the resolution ends.
        Event::LibraryLooked(looked) => {
            sight::show(state, Some(looked.player), &looked.cards);
            Ok(())
        }
        Event::CardsFound(found) => {
            sight::show(state, Some(found.player), &found.cards);
            Ok(())
        }
        Event::Revealed(revealed) => {
            sight::show(state, None, &revealed.cards);
            Ok(())
        }
        _ => apply_misc(state, event),
    }
}

fn apply_misc(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::TurnBegan { number, active } => {
            let (before, previous) = (state.turn.number, state.turn.active);
            state.turn.number = *number;
            state.turn.active = *active;
            state.turn.step = Step::Untap;
            state.turn.lands_played = 0;
            if before == 0 || state.opens_round(previous, *active) {
                state.turn.round += 1;
            }
            state.clear_turn_uses();
            state.priority = None;
            state.combat = crate::combat::CombatState::default();
            // A permanent its controller has held since this turn began is no longer held back (CR 302.6): who
            // controls it is read under the effects as they are now.
            let rested: Vec<ObjectId> = state
                .zone(Zone::Battlefield)
                .iter()
                .copied()
                .filter(|id| controller_of(state, *id) == Some(*active))
                .collect();
            for id in rested {
                if let Some(perm) = state
                    .objects
                    .get_mut(&id)
                    .and_then(|o| o.permanent.as_mut())
                {
                    perm.summoning_sick = false;
                    perm.observed = Some(*active);
                }
            }
        }
        Event::StepBegan { step } => {
            state.turn.step = *step;
            state.priority = None;
            // Each combat damage step divides its damage afresh: a double striker divides twice.
            state.combat.clear_assignments();
        }
        // `ManaBurned` is information in the log: the life it cost is its own `LifeLost`.
        Event::NothingToDo { .. } | Event::ManaBurned { .. } | Event::Cycled { .. } => {}
        Event::PriorityGranted { player } => {
            state.priority = Some(PriorityState {
                holder: *player,
                passed: BTreeSet::default(),
            });
        }
        Event::PriorityPassed { player } => pass_priority(state, *player)?,
        Event::PermanentTapped { object } => set_tapped(state, *object, true)?,
        Event::PermanentUntapped { object } => set_tapped(state, *object, false)?,
        Event::ManaAdded {
            player, mana, only, ..
        } => {
            pool_mut(state, *player)?.add_restricted(*mana, only.clone());
            reset_passes(state);
        }
        Event::ManaSpent {
            player,
            mana,
            restricted,
        } => {
            let pool = pool_mut(state, *player)?;
            let mut plain = mana.clone();
            for unit in restricted {
                if let Some(at) = plain.iter().position(|m| *m == unit.mana) {
                    plain.remove(at);
                }
                if !pool.spend_restricted(unit) {
                    return Err(invalid(format!(
                        "{player:?} spent {:?} mana restricted to something they do not have",
                        unit.mana
                    )));
                }
            }
            for m in &plain {
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
            let seat = player_mut(state, *player)?;
            seat.failed_draws = seat.failed_draws.saturating_add(1);
        }
        Event::EffectCreated { effect } => create_effect(state, effect)?,
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

/// Registers a continuous effect, noting what an ability it grants can do to the game.
fn create_effect(
    state: &mut GameState,
    effect: &crate::effects::ContinuousEffect,
) -> Result<(), EngineError> {
    if effect.layer != effect.modification.layer() {
        return Err(invalid("effect layer does not match its modification"));
    }
    if effect.duration == mtg_cards::modify::Duration::WhileSourceOnBattlefield
        && effect.source.is_none()
    {
        return Err(invalid(
            "an effect that lasts while its source stays has a source",
        ));
    }
    if state.effects.iter().any(|e| e.id == effect.id) {
        return Err(invalid(format!("duplicate effect {:?}", effect.id)));
    }
    state.next_effect = state.next_effect.max(effect.id.0 + 1);
    state.next_timestamp = state.next_timestamp.max(effect.timestamp + 1);
    if matches!(
        &effect.modification,
        Modification::AddKeyword(mtg_cards::keyword::Keyword::Protection(_))
    ) {
        state.possible.protection = true;
    }
    if let Modification::GrantAbility(granted) = &effect.modification {
        if matches!(**granted, mtg_cards::ability::Ability::Replacement { .. }) {
            state.possible.replacements = true;
        }
        if triggers_something(granted) {
            state.triggers_possible = true;
        }
        if has_state_trigger(granted) {
            state.possible.states = true;
        }
    }
    state.effects.push(effect.clone());
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
    player.failed_draws = 0;
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
