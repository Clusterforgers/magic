//! The single state transition: folding one [`Event`] into a [`GameState`].

#[cfg(test)]
mod active_ref;
mod combat;
#[cfg(test)]
mod crew_record;
#[cfg(test)]
mod distinct_names;
#[cfg(test)]
mod entered;
#[cfg(test)]
mod ep21_life_lost;
#[cfg(test)]
mod fe1a_atoms;
#[cfg(test)]
mod g13a_atoms;
#[cfg(test)]
mod g13b_left;
#[cfg(test)]
mod g8a_faces;
#[cfg(test)]
mod l292_counter_kinds;
mod opening;
#[cfg(test)]
mod p12_amounts;
#[cfg(test)]
mod p12_arrived;
mod r285_ruin;
#[cfg(test)]
mod r374_counts;
#[cfg(test)]
mod r3d_atoms;
mod resolution;
#[cfg(test)]
mod rmt8_atoms;
pub(crate) use resolution::step_done;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod w21a_atoms;
#[cfg(test)]
mod zone_tally;
mod zones;

use std::collections::BTreeSet;

use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of;
use crate::dk1c5_rooms::dk1c5_fold_unlocked as unlock;
use crate::effects::Modification;
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
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
    w2c_note_destroyed(state, event);
    w14b_note_exiled(state, event);
    crate::memory::forget_object(state, event);
    crate::triggers::g10c_note_crime(state, event);
    let folded = fold_event(state, event);
    // A read in the middle of the fold saw the state before the rest of the event: forget it.
    if changes_layers {
        state.layers.reset_if_built();
        state.rules_cache.reset_if_built();
        crate::combat::w7d_leave_on_control_change(state);
    }
    folded
}

#[allow(clippy::too_many_lines)]
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
        | Event::OpeningCardsPlayed { .. }
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
        | Event::ObjectLeftGame { .. } => {
            zones::apply(state, event)?;
            opening::dk26_exiled(state, event)
        }
        Event::GameRestarted { by, kept } => {
            crate::restart::fold(state, *by, kept);
            Ok(())
        }
        Event::CompanionRevealed { .. } | Event::CompanionBrought { .. } => {
            crate::companions::fold(state, event)
        }
        Event::AttackersDeclared { .. }
        | Event::BandFormed { .. }
        | Event::BlockersDeclared { .. }
        | Event::DamageAssigned { .. }
        | Event::DamageDealt { .. }
        | Event::CombatEnded
        | Event::JoinedAttack { .. }
        | Event::AttackRedirected { .. }
        | Event::RemovedFromCombat { .. }
        | Event::AttackersBlocked { .. }
        | Event::Regenerated { .. }
        | Event::DamageRemoved { .. }
        | Event::DamageCleared => {
            combat::apply(state, event)?;
            crate::rmt8_turns::mark_fought(state, event);
            Ok(())
        }
        Event::AbilityPutOnStack(_)
        | Event::AbilityActivated { .. }
        | Event::ResolutionBegan { .. }
        | Event::ChoiceMade { .. }
        | Event::StepDone { .. }
        | Event::ResolutionEnded { .. } => resolution::apply(state, event),
        // A note for the table: the game state does not change.
        // A prevention changes nothing of its own; triggers read it.
        Event::AbilityLeftToTable(_)
        | Event::ManaAbilityActivated { .. }
        | Event::ByHandDue(_)
        | Event::DamagePrevented { .. }
        // The `Revealed` before it showed the card; triggers read it from the log.
        | Event::FirstDrawRevealed { .. } => Ok(()),
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
        Event::Paired { object, with } => crate::soulbond::fold_paired(state, *object, *with),
        Event::Unpaired { object, with } => crate::soulbond::fold_unpaired(state, *object, *with),
        Event::TokenCreated(token) => crate::tokens::fold(state, token),
        Event::CardConjured(card) => crate::conjure::fold(state, card),
        Event::BecameCopy(copy) => crate::copies::fold(state, copy),
        Event::SpellCopied(copy) => crate::copies::fold_spell(state, copy),
        Event::CardCopied(copy) => crate::copies::fold_card(state, copy),
        Event::CopyCeased { object } => crate::copies::fold_card_ceased(state, *object),
        Event::AbilityCopied(copy) => crate::copies::fold_ability(state, copy),
        Event::TargetsChanged(change) => crate::retargeted::fold(state, change),
        Event::Remembered(change) => {
            crate::memory::fold(state, change);
            Ok(())
        }
        ended @ Event::CopyEnded { .. } => crate::copies::fold_ended(state, ended),
        Event::Prepared { object } => set_prepared(state, *object, true),
        Event::Unprepared { object } => set_prepared(state, *object, false),
        Event::Suspected { object } => set_suspected(state, *object, true),
        Event::Unsuspected { object } => set_suspected(state, *object, false),
        Event::Solved { object } => set_solved(state, *object),
        Event::Harnessed { object } => crate::rmt8_turns::set_harnessed(state, *object),
        Event::Crewed { .. } => tally_crewed(state, event),
        // The change follows as events of its own; the action restarts the round, so the other seats see it, and a look
        // by hand records what the seat learned.
        Event::ManualAction { player, action } => {
            reset_passes(state);
            match **action {
                crate::manual::ManualAction::LookTop { count } => {
                    sight::look_by_hand(state, *player, count);
                }
                crate::manual::ManualAction::Search {} => {
                    let all = state.zone(Zone::Library(*player)).len();
                    sight::look_by_hand(state, *player, u32::try_from(all).unwrap_or(u32::MAX));
                }
                _ => {}
            }
            crate::manual_show::apply(state, *player, action);
            crate::manual_control::apply(state, action);
            Ok(())
        }
        Event::ManualRolled { .. } => {
            // A roll spends a substream of the generator, as a shuffle does.
            state.rng_index += 1;
            Ok(())
        }
        Event::Scried(scried) => {
            zones::scry(state, scried)?;
            state.tally(scried.player, Tally::Scried);
            opening::scried(state, scried.player)?;
            let seen: Vec<ObjectId> = scried.top.iter().chain(&scried.bottom).copied().collect();
            sight::show(state, Some(scried.player), &seen);
            Ok(())
        }
        Event::Surveilled(surveilled) => zones::surveil(state, surveilled),
        Event::Investigated { player } => {
            state.tally(*player, crate::state::Tally::L31aInvestigated);
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
        Event::LibraryInserted {
            player,
            cards,
            below,
        } => zones::insert(state, *player, cards, *below),
        Event::LibraryLooked(_)
        | Event::HandLooked(_)
        | Event::CardsFound(_)
        | Event::Revealed(_) => {
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
        Event::HandLooked(looked) => sight::show(state, Some(looked.player), &looked.cards),
        Event::CardsFound(found) => {
            sight::show(state, Some(found.player), &found.cards);
            if let Some(resolving) = state.resolving.as_mut() {
                resolving.searched.push((found.player, found.zone));
            }
        }
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
        Event::EnduringStoryGained { player } => {
            state.enduring_story.insert(*player);
        }
        Event::SpeedChanged { player, speed } => crate::speed::set(state, *player, *speed),
        Event::RingBearerLost { player } => {
            if let Some(ring) = state.ring.get_mut(player) {
                ring.bearer = None;
            }
        }
        Event::RingTempted {
            player,
            level,
            bearer,
        } => {
            let chosen = bearer
                .and_then(|id| state.object(id))
                .map(crate::object::GameObject::reference);
            let ring = state.ring.entry(*player).or_default();
            ring.level = *level;
            if chosen.is_some() {
                ring.bearer = chosen;
            }
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
    // The turn that is ending is kept before its counts are cleared.
    if before != 0 {
        let ending = state.turn.active;
        state.summarize_turn(ending);
        state.r379_turn_ended(ending, before);
    }
    if extra {
        if state.extra_turns.pop().map(|turn| turn.player) != Some(active) {
            return Err(invalid(format!("{active:?} has no extra turn to take")));
        }
    } else {
        state.turn.regular = active;
    }
    state.rmt8_count_turn(active);
    state.turn.number = number;
    state.turn.active = active;
    state.turn.extra = extra;
    state.turn.step = Step::Untap;
    state.turn.lands_played = 0;
    state.turn.combats = 0;
    state.turn.ahead.clear();
    state.turn.r5500_lone.clear();
    state.turn.r5500_in_lone = false;
    state.turn.r306_steps.clear();
    state.turn.r306_seen = 0;
    state.turn.r306_again = 0;
    state.turn.ending = false;
    crate::player_control::turn_began(state, number, active);
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
fn add_phases(state: &mut GameState, steps: &[Step], lone: bool) -> Result<(), EngineError> {
    let turn = &mut state.turn;
    if turn.ahead.is_empty() {
        // After a step on its own, the phase that follows the one it came after is still held.
        let held = turn.step.phase().next_start();
        turn.ahead
            .push(held.ok_or_else(|| invalid("no phase follows the ending phase"))?);
        if !turn.r5500_lone.is_empty() {
            turn.r5500_lone.push(false);
        }
    }
    turn.r5500_add_ahead(steps, lone);
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
        Event::PlayerControlGained {
            player,
            by,
            extra_turn,
        } => crate::player_control::gain(state, *player, *by, *extra_turn),
        Event::ExtraTurnSkipped { player } => {
            if state.extra_turns.pop().map(|turn| turn.player) != Some(*player) {
                return Err(invalid(format!("{player:?} has no extra turn to skip")));
            }
        }
        Event::PhasesAdded { steps, lone } => add_phases(state, steps, *lone)?,
        Event::StepAdded { step } => state.turn.r306_steps.insert(0, *step),
        Event::StepBegan { step } => {
            // A step added after the one that ends begins next, and its entry is spent.
            let from_added = state.turn.r306_steps.first() == Some(step);
            if from_added {
                state.turn.r306_steps.remove(0);
            }
            state.turn.r306_note_step(*step);
            // A phase added after the current one begins as its entry says, and the entry is spent (CR 500.8).
            if state.turn.ahead.first() == Some(step) {
                state.turn.r5500_begin_ahead();
            } else if !from_added {
                state.turn.r5500_in_lone = false;
            }
            if *step == Step::Upkeep {
                state.rmt8_mark_upkeep();
            }
            if *step == Step::BeginCombat {
                state.turn.combats += 1;
                state.begin_combat_log();
            }
            // "Each of your draw steps" counts the draws of each draw step afresh, an added one included.
            if *step == Step::Draw {
                state
                    .tallies
                    .players
                    .retain(|(_, kind), _| *kind != Tally::C6BlkDrawStepDrawn);
            }
            // A cleanup step that begins during a resolution ends the turn by an effect (CR 723).
            state.turn.ending = *step == Step::Cleanup && state.resolving.is_some();
            state.turn.step = *step;
            state.priority = None;
            // Each combat damage step divides its damage afresh: a double striker divides twice.
            state.combat.clear_assignments();
        }
        _ => {}
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn apply_misc(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::TurnBegan { .. }
        | Event::ExtraTurnAdded { .. }
        | Event::PlayerControlGained { .. }
        | Event::ExtraTurnSkipped { .. }
        | Event::PhasesAdded { .. }
        | Event::StepAdded { .. }
        | Event::StepBegan { .. } => apply_turn(state, event)?,
        Event::SpellControlGained { object, player } => {
            let spell = state
                .objects
                .get_mut(object)
                .filter(|spell| spell.zone() == Zone::Stack)
                .ok_or_else(|| invalid(format!("{object:?} is no spell on the stack")))?;
            spell.controller = *player;
        }
        // `ManaBurned` is information in the log: the life it cost is its own `LifeLost`.
        Event::NothingToDo { .. }
        | Event::AnnouncementRewound { .. }
        | Event::TookBack { .. }
        | Event::ManaBurned { .. }
        | Event::Explored { .. }
        | Event::Connived { .. }
        | Event::Amassed { .. }
        | Event::Adapted { .. }
        | Event::Bolstered { .. }
        | Event::Learned { .. }
        | Event::EvidenceCollected { .. }
        | Event::Discovered { .. }
        | Event::DreadManifested { .. }
        | Event::GiftGiven { .. }
        | Event::UpkeepSettled { .. }
        | Event::Foraged { .. }
        | Event::Clashed { .. }
        | Event::Proliferated { .. } => {}
        Event::VotesFinished { ballots } => state.r379_votes.push(ballots.clone()),
        Event::ChosenAtRandom { .. }
        | Event::PlayersChosenAtRandom { .. }
        | Event::Sought { .. }
        | Event::DieRolled { .. }
        | Event::CoinFlipped { .. } => {
            c11e_draw_random(state, event);
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
        | Event::RingTempted { .. }
        | Event::RingBearerLost { .. }
        | Event::CitysBlessingGained { .. }
        | Event::EnduringStoryGained { .. }
        | Event::SpeedChanged { .. } => designate(state, event),

        Event::PermanentTapped { object } => {
            set_tapped(state, *object, true)?;
            if let Some(tapped) = state.object(*object).map(GameObject::reference) {
                state.tally_object(tapped, None, Tally::E16aTapped);
            }
        }
        Event::PermanentUntapped { object } => set_tapped(state, *object, false)?,
        Event::PermanentPhasedOut { .. } => crate::phasing::w11b_fold_out_event(state, event)?,
        Event::PermanentPhasedIn { object } => crate::phasing::fold_in(state, *object)?,
        Event::Monstrous { object, .. } => set_monstrous(state, *object)?,
        Event::Devoured { object, count } => w7e_set_devoured(state, *object, *count)?,
        Event::ClassLevelGained { object, level, .. } => set_level(state, *object, *level)?,
        Event::TurnedFaceUp { object, player, .. } => {
            turn_face_up(state, *object)?;
            state.record(
                mtg_cards::value::TallyEvent::TurnedFaceUp,
                Some(*player),
                (*object, None),
                1,
            );
        }
        Event::TurnedFaceDown { object, .. } => crate::rmt8_turns::turn_face_down(state, *object)?,
        Event::CommanderColorChosen { object, color, .. } => {
            state.commander_colors.retain(|(card, _)| card != object);
            state.commander_colors.push((*object, *color));
        }
        Event::FaceDownLooked { object, player } => {
            crate::sight::look_face_down(state, *player, *object);
        }
        Event::DoorUnlocked { object, door, .. } => unlock(state, *object, *door)?,
        Event::DoorLocked { object, door, .. } => {
            crate::dk1c5_rooms::r281_fold_locked(state, *object, *door)?;
        }
        Event::ManaAdded {
            player,
            mana,
            only,
            lasts,
            source,
        } => {
            let from = source.map(|id| crate::spend::source_kind(state, id));
            pool_mut(state, *player)?.add_from(*mana, only.clone(), *lasts, from);
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
        Event::ManaPoolEmptied {
            player,
            through,
            kept,
            becomes,
        } => pool_mut(state, *player)?.empty_keeping(*through, kept, *becomes),
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
        Event::BecameSpirit { player, emblem } => crate::spirits::fold(state, *player, *emblem)?,
        Event::PhaseBegan { player, emblem } => crate::phases::fold(state, *player, *emblem)?,
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
            | mtg_cards::modify::Duration::WhileSourceTapped
            | mtg_cards::modify::Duration::WhileYouControlSourceTapped
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
        Modification::AddKeyword(
            mtg_cards::keyword::Keyword::Protection(_)
                | mtg_cards::keyword::Keyword::ProtectionSparing { .. }
        )
    ) {
        state.possible.protection = true;
    }
    // A card that gains suspend in exile triggers from there (CR 702.62a).
    if matches!(
        &effect.modification,
        Modification::AddKeyword(mtg_cards::keyword::Keyword::Suspend { .. })
    ) {
        state.possible.elsewhere = true;
        state.triggers_possible = true;
        state.statics_possible = true;
    }
    // A spell that gains rebound on the stack is exiled by a replacement effect as it resolves (CR 702.88a).
    if matches!(
        &effect.modification,
        Modification::AddKeyword(mtg_cards::keyword::Keyword::Rebound)
    ) {
        state.possible.replacements = true;
    }
    if let Modification::GrantAbility(granted) = &effect.modification {
        if matches!(**granted, mtg_cards::ability::Ability::Replacement { .. }) {
            state.possible.replacements = true;
        }
        if crate::ability_reader::is_rule(granted) {
            state.rules_possible = true;
        }
        if crate::ability_reader::is_cost_modifier(granted) {
            state.possible.costs = true;
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

/// A Two-Headed Giant team has one life total (CR 810.9): a change to one player's total is the team's, so the
/// teammates hold the same number.
pub(crate) fn share_life(state: &mut GameState, id: PlayerId) {
    let Some(life) = state.player(id).map(crate::player::Player::life) else {
        return;
    };
    for mate in state.life_sharers(id) {
        if let Ok(player) = player_mut(state, mate) {
            player.life = life;
        }
    }
}

/// Takes `amount` off a life total as a cost; a player cannot pay more life than they have (CR 119.4).
fn pay_life(state: &mut GameState, id: PlayerId, amount: u32) -> Result<(), EngineError> {
    let player = player_mut(state, id)?;
    match i32::try_from(amount) {
        Ok(paid) if paid > 0 && paid <= player.life => {
            player.life -= paid;
            state.tally_loss(id, amount);
            share_life(state, id);
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
    // An amount past the largest life total changes it as far as it goes.
    match i32::try_from(amount).unwrap_or(i32::MAX) {
        change if change > 0 => {
            player.life = if gain {
                player.life.saturating_add(change)
            } else {
                player.life.saturating_sub(change)
            };
            if !gain {
                state.tally_loss(id, amount);
            }
            share_life(state, id);
            Ok(())
        }
        _ => Err(invalid(format!("{id:?} cannot change life by {amount}"))),
    }
}

/// A random pick, roll or flip spends a substream of the generator, as a shuffle does; a die rolled while a spell or
/// ability resolves is kept for "the result".
fn c11e_draw_random(state: &mut GameState, event: &Event) {
    state.rng_index += 1;
    if let (Event::DieRolled { result, .. }, Some(resolving)) = (event, state.resolving.as_mut()) {
        resolving.rolled.push(*result);
    }
}

/// A permanent destroyed while a spell or ability resolves is kept, as the object it was, for "destroyed this way".
fn w2c_note_destroyed(state: &mut GameState, event: &Event) {
    let Event::ZoneChanged {
        object,
        from: Zone::Battlefield,
        cause: ZoneCause::Destroyed | ZoneCause::Buried,
        ..
    } = event
    else {
        return;
    };
    let Some(was) = state.object(*object).map(GameObject::reference) else {
        return;
    };
    if let Some(resolving) = state.resolving.as_mut() {
        resolving.destroyed.push(was);
    }
}

/// A permanent exiled while a spell or ability resolves is kept, as the object it was, for "exiled this way".
fn w14b_note_exiled(state: &mut GameState, event: &Event) {
    let Event::ZoneChanged {
        object,
        from: Zone::Battlefield,
        to: Zone::Exile,
        ..
    } = event
    else {
        return;
    };
    let Some(was) = state.object(*object).map(GameObject::reference) else {
        return;
    };
    if let Some(resolving) = state.resolving.as_mut() {
        resolving.w14b_exiled.push(was);
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
    if state.turn.ending {
        return;
    }
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
        Event::Exploited { .. } | Event::Enlisted { .. } | Event::PrizeClaimed { .. } => Ok(()),
        Event::EmblemCreated(emblem) => crate::w6b_emblems::fold(state, emblem),
        Event::Exerted { object, player } => {
            crate::w11a_exert::w11a_fold_exerted(state, *object, *player)
        }
        Event::ExertEnded { object, player } => {
            crate::w11a_exert::w11a_fold_ended(state, *object, *player)
        }
        Event::Ventured(ventured) => crate::w11d_dungeon::w11d_fold_ventured(state, ventured),
        Event::AttractionDeckShuffled { player, order } => {
            state.rng_index += 1;
            crate::un33_attractions::fold_shuffled(state, *player, order)
        }
        Event::RolledToVisit {
            player, visited, ..
        } => {
            crate::un33_attractions::fold_rolled(state, *player, visited);
            Ok(())
        }
        Event::StickerSheetsChosen { player, sheets } => {
            state.rng_index += 1;
            crate::un34_stickers::fold_chosen(state, *player, sheets)
        }
        Event::StickerPut {
            player,
            object,
            sheet,
            sticker,
        } => crate::un34_stickers::fold_put(
            state,
            crate::un34_stickers::StickerAt {
                player: *player,
                sheet: *sheet,
                sticker: *sticker,
            },
            *object,
        ),
        Event::InitiativeTaken { player, object } => {
            crate::w15a_initiative::w15a_fold_taken(state, *player, *object)
        }
        Event::DungeonCompleted { player, object } => {
            crate::w11d_dungeon::w11d_fold_completed(state, *player, *object)
        }
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

/// A permanent becomes suspected or stops being so: it was not (or was), and it is a permanent.
fn set_suspected(state: &mut GameState, id: ObjectId, suspected: bool) -> Result<(), EngineError> {
    let perm = state
        .objects
        .get_mut(&id)
        .and_then(|object| object.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    if perm.designations.suspected == suspected {
        return Err(invalid(format!(
            "{id:?} is {}suspected already",
            if suspected { "" } else { "un" }
        )));
    }
    perm.designations.suspected = suspected;
    state.possible.suspected |= suspected;
    Ok(())
}

/// A permanent becomes solved (CR 719.3): it was not, and it is a permanent.
fn set_solved(state: &mut GameState, id: ObjectId) -> Result<(), EngineError> {
    let perm = state
        .objects
        .get_mut(&id)
        .and_then(|object| object.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    if perm.designations.solved {
        return Err(invalid(format!("{id:?} is solved already")));
    }
    perm.designations.solved = true;
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
/// A permanent that entered devouring keeps how many it devoured (CR 702.82).
fn w7e_set_devoured(state: &mut GameState, id: ObjectId, count: u32) -> Result<(), EngineError> {
    let perm = state
        .objects
        .get_mut(&id)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    perm.designations.devoured = count;
    Ok(())
}

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
