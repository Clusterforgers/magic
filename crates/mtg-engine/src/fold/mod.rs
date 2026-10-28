//! The single state transition: folding one [`Event`] into a [`GameState`].

mod combat;
#[cfg(test)]
mod crew_record;
#[cfg(test)]
mod entered;
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
use crate::object::{GameObject, ObjectRef};
use crate::player::{LossReason, Player, PlayerStatus};
use crate::priority::PriorityState;
use crate::sight;
use crate::state::{GameResult, GameState, Stage, Tally};
use crate::triggers::{has_state_trigger, triggers_something};
use crate::turn::{ExtraTurn, Step};
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
        | Event::Mutated { .. }
        | Event::CardParted { .. }
        | Event::CommanderStayed { .. }
        | Event::ObjectLeftGame { .. } => zones::apply(state, event),
        Event::CompanionRevealed { .. } | Event::CompanionBrought { .. } => {
            crate::companions::fold(state, event)
        }
        Event::AttackersDeclared { .. }
        | Event::BlockersDeclared { .. }
        | Event::DamageAssigned { .. }
        | Event::DamageDealt { .. }
        | Event::CombatEnded
        | Event::JoinedAttack { .. }
        | Event::Regenerated { .. }
        | Event::DamageRemoved { .. }
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
        Event::AttachedToPlayer { object, player } => {
            crate::attach::fold_attached_to_player(state, *object, *player)
        }
        Event::Unattached { object } => crate::attach::fold_unattached(state, *object),
        Event::TokenCreated(token) => crate::tokens::fold(state, token),
        Event::BecameCopy(copy) => crate::copies::fold(state, copy),
        Event::SpellCopied(copy) => crate::copies::fold_spell(state, copy),
        Event::CardCopied(copy) => crate::copies::fold_card(state, copy),
        Event::CopyCeased { object } => crate::copies::fold_card_ceased(state, *object),
        Event::AbilityCopied(copy) => crate::copies::fold_ability(state, copy),
        Event::CopyEnded { object } => crate::copies::fold_ended(state, *object),
        Event::Prepared { object } => set_prepared(state, *object, true),
        Event::Unprepared { object } => set_prepared(state, *object, false),
        Event::Crewed { .. } => tally_crewed(state, event),
        // The change follows as events of its own; the action only restarts the round, so the other seats see it.
        Event::ManualAction { .. } => {
            reset_passes(state);
            Ok(())
        }
        Event::Scried(scried) => {
            zones::scry(state, scried)?;
            opening::scried(state, scried.player)?;
            let seen: Vec<ObjectId> = scried.top.iter().chain(&scried.bottom).copied().collect();
            sight::show(state, Some(scried.player), &seen);
            Ok(())
        }
        Event::Surveilled(surveilled) => zones::surveil(state, surveilled),
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
        Event::LibraryLooked(_) | Event::CardsFound(_) | Event::Revealed(_) => {
            show_learned(state, event);
            Ok(())
        }
        _ => apply_misc(state, event),
    }
}

/// What a player learned changes nothing in the game but who may identify the cards until the resolution ends.
fn show_learned(state: &mut GameState, event: &Event) {
    match event {
        Event::LibraryLooked(looked) => sight::show(state, Some(looked.player), &looked.cards),
        Event::CardsFound(found) => sight::show(state, Some(found.player), &found.cards),
        Event::Revealed(revealed) => sight::show(state, None, &revealed.cards),
        _ => {}
    }
}

/// The designations of the game: day or night (CR 726), the monarch (CR 724) and the city's blessing (CR 702.131).
fn designate(state: &mut GameState, event: &Event) {
    match event {
        Event::DayNightChanged { to } => state.day_night = Some(*to),
        Event::MonarchChanged { player } => crate::monarch::set(state, Some(*player)),
        Event::CitysBlessingGained { player } => {
            state.citys_blessing.insert(*player);
        }
        _ => {}
    }
}

/// The permanents `active` controls as their turn begins. One it has held since its previous turn began is no longer
/// held back (CR 302.6); one that was held back is recorded as new to them, which echo reads in the upkeep
/// (`PermanentState::is_new_to`). Who controls a permanent is read under the effects as they are now.
fn settle_permanents(state: &mut GameState, active: PlayerId) {
    let controlled: Vec<ObjectId> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| controller_of(state, *id) == Some(active))
        .collect();
    for id in controlled {
        if let Some(perm) = state
            .objects
            .get_mut(&id)
            .and_then(|o| o.permanent.as_mut())
        {
            perm.new_to = perm.sick_for(active).then_some(active);
            perm.summoning_sick = false;
            perm.observed = Some(active);
        }
    }
}

/// A turn begins: its number and active player, the first step, and a fresh combat. An extra turn takes the latest
/// one added (CR 500.7) and keeps the seat of the regular turn it came after; it opens no round.
fn begin_turn(
    state: &mut GameState,
    number: u32,
    active: PlayerId,
    extra: bool,
) -> Result<(), EngineError> {
    let (before, previous) = (state.turn.number, state.turn.regular);
    if extra {
        if state.extra_turns.pop().map(|turn| turn.player) != Some(active) {
            return Err(invalid(format!("{active:?} has no extra turn to take")));
        }
    } else {
        state.turn.regular = active;
    }
    state.turn.number = number;
    state.turn.active = active;
    state.turn.extra = extra;
    state.turn.step = Step::Untap;
    state.turn.lands_played = 0;
    state.turn.combats = 0;
    state.turn.ahead.clear();
    if !extra && (before == 0 || state.opens_round(previous, active)) {
        state.turn.round += 1;
    }
    state.clear_turn_uses();
    state.priority = None;
    state.combat = crate::combat::CombatState::default();
    settle_permanents(state, active);
    Ok(())
}

/// Phases are added after the current one (CR 500.8): they begin before any added earlier, and the phase of the
/// regular sequence that follows the current one comes after all of them.
fn add_phases(state: &mut GameState, steps: &[Step]) -> Result<(), EngineError> {
    let turn = &mut state.turn;
    if turn.ahead.is_empty() {
        let held = turn.step.phase().next_start();
        turn.ahead
            .push(held.ok_or_else(|| invalid("no phase follows the ending phase"))?);
    }
    turn.ahead.splice(0..0, steps.iter().copied());
    Ok(())
}

/// The events of the turn sequence: turns begin, extra turns and phases are added, steps begin.
fn apply_turn(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::TurnBegan {
            number,
            active,
            extra,
        } => begin_turn(state, *number, *active, *extra)?,
        Event::ExtraTurnAdded { player, skip_untap } => state.extra_turns.push(ExtraTurn {
            player: *player,
            skip_untap: *skip_untap,
        }),
        Event::ExtraTurnSkipped { player } => {
            if state.extra_turns.pop().map(|turn| turn.player) != Some(*player) {
                return Err(invalid(format!("{player:?} has no extra turn to skip")));
            }
        }
        Event::PhasesAdded { steps } => add_phases(state, steps)?,
        Event::StepBegan { step } => {
            // A phase added after the current one begins as its entry says, and the entry is spent (CR 500.8).
            if state.turn.ahead.first() == Some(step) {
                state.turn.ahead.remove(0);
            }
            if *step == Step::BeginCombat {
                state.turn.combats += 1;
            }
            // "Each of your draw steps" counts the draws of each draw step afresh, an added one included.
            if *step == Step::Draw {
                state
                    .tallies
                    .players
                    .retain(|(_, kind), _| *kind != Tally::C6BlkDrawStepDrawn);
            }
            state.turn.step = *step;
            state.priority = None;
            // Each combat damage step divides its damage afresh: a double striker divides twice.
            state.combat.clear_assignments();
        }
        _ => {}
    }
    Ok(())
}

fn apply_misc(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::TurnBegan { .. }
        | Event::ExtraTurnAdded { .. }
        | Event::ExtraTurnSkipped { .. }
        | Event::PhasesAdded { .. }
        | Event::StepBegan { .. } => apply_turn(state, event)?,
        // `ManaBurned` is information in the log: the life it cost is its own `LifeLost`.
        Event::NothingToDo { .. }
        | Event::ManaBurned { .. }
        | Event::Explored { .. }
        | Event::Connived { .. }
        | Event::Amassed { .. }
        | Event::Adapted { .. }
        | Event::Bolstered { .. }
        | Event::Learned { .. }
        | Event::Proliferated { .. } => {}
        // Each random pick, roll and flip spends a substream of the generator, as a shuffle does.
        Event::ChosenAtRandom { .. } | Event::DieRolled { .. } | Event::CoinFlipped { .. } => {
            state.rng_index += 1;
        }
        Event::Cycled { player, .. } => state.tally(*player, Tally::Cycled),
        Event::PriorityGranted { player } => {
            state.priority = Some(PriorityState {
                holder: *player,
                passed: BTreeSet::default(),
            });
        }
        Event::PriorityPassed { player } => pass_priority(state, *player)?,
        Event::Transformed { object, face } => set_face(state, *object, *face)?,
        Event::Flipped { object } => flip(state, *object)?,
        Event::DayNightChanged { .. }
        | Event::MonarchChanged { .. }
        | Event::CitysBlessingGained { .. } => designate(state, event),

        Event::PermanentTapped { object } => set_tapped(state, *object, true)?,
        Event::PermanentUntapped { object } => set_tapped(state, *object, false)?,
        Event::PermanentPhasedOut {
            object,
            controller,
            indirect,
        } => crate::phasing::fold_out(state, *object, *controller, *indirect)?,
        Event::PermanentPhasedIn { object } => crate::phasing::fold_in(state, *object)?,
        Event::Monstrous { object, .. } => set_monstrous(state, *object)?,
        Event::ClassLevelGained { object, level, .. } => set_level(state, *object, *level)?,
        Event::TurnedFaceUp { object, .. } => turn_face_up(state, *object)?,
        Event::ManaAdded {
            player,
            mana,
            only,
            lasts,
            ..
        } => {
            pool_mut(state, *player)?.add_restricted(*mana, only.clone(), *lasts);
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
        Event::ManaPoolEmptied { player, through } => pool_mut(state, *player)?.empty(*through),
        Event::LifePaid { player, amount } => pay_life(state, *player, *amount)?,
        Event::LifeGained { player, amount } => gain_life(state, *player, *amount)?,
        Event::LifeLost { player, amount } => lose_life(state, *player, *amount)?,
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
        other => return c6_kw_fold_rest(state, other),
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
    if matches!(
        effect.duration,
        mtg_cards::modify::Duration::WhileSourceOnBattlefield
            | mtg_cards::modify::Duration::WhileYouControlSource
    ) && effect.source.is_none()
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
            state.tally_loss(id, amount);
            Ok(())
        }
        _ => Err(invalid(format!(
            "{id:?} cannot pay {amount} life with {}",
            player.life
        ))),
    }
}

/// Adds `amount` to a life total, or takes it off; a total may go to 0 or below, which state-based actions judge.
/// A player gains life, which counts for "the second time this turn".
fn gain_life(state: &mut GameState, id: PlayerId, amount: u32) -> Result<(), EngineError> {
    change_life(state, id, amount, true)?;
    state.tally(id, Tally::Gained);
    state.tally_by(id, Tally::GainedTotal, amount);
    Ok(())
}

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
            if !gain {
                state.tally_loss(id, amount);
            }
            Ok(())
        }
        _ => Err(invalid(format!("{id:?} cannot change life by {amount}"))),
    }
}

/// A player loses life to an effect: the loss, and the sum "the life lost this way" reads while a spell or ability resolves.
fn lose_life(state: &mut GameState, id: PlayerId, amount: u32) -> Result<(), EngineError> {
    change_life(state, id, amount, false)?;
    if let Some(resolving) = state.resolving.as_mut() {
        resolving.lost = resolving.lost.saturating_add(amount);
    }
    Ok(())
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

/// A permanent becomes monstrous (CR 701.37): it was not, and it is a permanent.
fn set_monstrous(state: &mut GameState, id: ObjectId) -> Result<(), EngineError> {
    let perm = state
        .objects
        .get_mut(&id)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    if perm.designations.monstrous {
        return Err(invalid(format!("{id:?} is already monstrous")));
    }
    perm.designations.monstrous = true;
    Ok(())
}

/// The events no other fold names: becoming renowned, and `Exploited`, information in the log whose sacrifice is its own
/// `ZoneChanged`; any other is a bug.
fn c6_kw_fold_rest(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::Renowned { object, .. } => c6_kw_set_renowned(state, *object),
        Event::Exploited { .. } => Ok(()),
        other => Err(invalid(format!("unhandled event {other:?}"))),
    }
}

/// A permanent becomes renowned (CR 702.112): it was not, and it is a permanent.
fn c6_kw_set_renowned(state: &mut GameState, id: ObjectId) -> Result<(), EngineError> {
    let perm = state
        .objects
        .get_mut(&id)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    if perm.designations.renowned {
        return Err(invalid(format!("{id:?} is already renowned")));
    }
    perm.designations.renowned = true;
    Ok(())
}

/// A prepare card on the battlefield becomes prepared or stops being so: it was not (or was), and it is a permanent of
/// the `prepare` layout.
fn set_prepared(state: &mut GameState, id: ObjectId, prepared: bool) -> Result<(), EngineError> {
    let object = state
        .objects
        .get_mut(&id)
        .ok_or_else(|| invalid(format!("{id:?} does not exist")))?;
    if object.def.layout != mtg_cards::definition::Layout::Prepare {
        return Err(invalid(format!("{id:?} is not a prepare card")));
    }
    let perm = object
        .permanent
        .as_mut()
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    if perm.designations.prepared == prepared {
        return Err(invalid(format!(
            "{id:?} is {}prepared already",
            if prepared { "" } else { "un" }
        )));
    }
    perm.designations.prepared = prepared;
    Ok(())
}

/// A Vehicle becomes crewed or a Mount saddled: counted for the object, which is a permanent, and the creatures still on
/// the battlefield that were tapped to pay are recorded for it.
fn tally_crewed(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    let Event::Crewed {
        object: id,
        creatures,
        saddle,
        ..
    } = event
    else {
        return Err(invalid("not a crew".to_owned()));
    };
    let (id, saddle) = (*id, *saddle);
    let object = state
        .object(id)
        .map(GameObject::reference)
        .ok_or_else(|| invalid(format!("{id:?} is not an object")))?;
    state.tally_object(
        object,
        None,
        if saddle {
            Tally::Saddled
        } else {
            Tally::Crewed
        },
    );
    let crew: Vec<ObjectRef> = creatures
        .iter()
        .filter_map(|creature| state.object(*creature))
        .filter(|creature| creature.zone() == Zone::Battlefield)
        .map(GameObject::reference)
        .collect();
    state
        .tallies
        .crews
        .extend(crew.into_iter().map(|creature| (object, saddle, creature)));
    Ok(())
}

/// A Class gains a level (CR 716.2): the level is higher than the one it has, and it is a permanent.
fn set_level(state: &mut GameState, id: ObjectId, level: u32) -> Result<(), EngineError> {
    let perm = state
        .objects
        .get_mut(&id)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    if level <= perm.level() {
        return Err(invalid(format!("{id:?} is level {} already", perm.level())));
    }
    perm.designations.level = level;
    Ok(())
}

/// A face-down permanent is turned face up (CR 702.37e): it is its card from now on.
fn turn_face_up(state: &mut GameState, id: ObjectId) -> Result<(), EngineError> {
    let object = state
        .objects
        .get_mut(&id)
        .filter(|object| object.zone == crate::zones::Zone::Battlefield)
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    if !object
        .face_down
        .is_some_and(crate::object::FaceDown::shows_characteristics)
    {
        return Err(invalid(format!("{id:?} is not face down")));
    }
    object.face_down = None;
    Ok(())
}

/// A transforming double-faced permanent is turned to `face` (CR 701.28); its counters, attachments and the rest of what it
/// remembers stay, since it is the same object.
fn set_face(state: &mut GameState, id: ObjectId, face: Option<u8>) -> Result<(), EngineError> {
    let object = state
        .objects
        .get_mut(&id)
        .ok_or_else(|| invalid(format!("{id:?} does not exist")))?;
    if object.def.layout != mtg_cards::definition::Layout::Transform {
        return Err(invalid(format!(
            "{id:?} is not a transforming double-faced card"
        )));
    }
    let permanent = object
        .permanent
        .as_mut()
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    permanent.face = face;
    Ok(())
}

/// A flip card on the battlefield is flipped (CR 710.4): the same object with its bottom half up, which it stays until it
/// leaves the battlefield.
fn flip(state: &mut GameState, id: ObjectId) -> Result<(), EngineError> {
    let object = state
        .objects
        .get_mut(&id)
        .ok_or_else(|| invalid(format!("{id:?} does not exist")))?;
    if object.def.layout != mtg_cards::definition::Layout::Flip {
        return Err(invalid(format!("{id:?} is not a flip card")));
    }
    let permanent = object
        .permanent
        .as_mut()
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    permanent.face = Some(1);
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
    if state.monarch == Some(id) {
        let successor = crate::monarch::successor(state, id);
        crate::monarch::set(state, successor);
    }
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
