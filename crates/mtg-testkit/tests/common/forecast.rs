//! Goal: shared check that a combat forecast equals what the engine then did (this is not a test module itself).

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::combat::DamageTarget;
use mtg_engine::forecast::CombatForecast;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameState};

/// Whether `events` (produced from a state in `step_before`) include the dealing of regular combat damage.
///
/// Damage and the priority grant that follows it are one batch, so a grant inside the damage step
/// marks the batch even when every creature had 0 power and no `DamageDealt` exists.
pub fn damage_was_dealt(step_before: Step, events: &[Event]) -> bool {
    damage_was_dealt_in(Step::CombatDamage, step_before, events)
}

/// Whether `events` (produced from a state in `step_before`) include the dealing of the damage of `damage_step`,
/// the first-strike step or the regular one.
pub fn damage_was_dealt_in(damage_step: Step, step_before: Step, events: &[Event]) -> bool {
    let mut in_damage_step = step_before == damage_step;
    for event in events {
        match event {
            Event::StepBegan { step } => in_damage_step = *step == damage_step,
            Event::PriorityGranted { .. } if in_damage_step => return true,
            _ => {}
        }
    }
    false
}

/// Compares `forecast` (taken right before the answer) with the events that answer produced.
///
/// A creature that strikes in two steps deals damage in two events, which the forecast holds as one packet,
/// so the events are totalled by source and recipient first. Returns the first difference as text.
pub fn disagreement(
    forecast: &CombatForecast,
    events: &[Event],
    after: &GameState,
) -> Option<String> {
    let mut totals: BTreeMap<(ObjectId, DamageTarget), u32> = BTreeMap::new();
    let mut to_players: BTreeMap<PlayerId, u32> = BTreeMap::new();
    let mut to_objects: BTreeMap<ObjectId, u32> = BTreeMap::new();
    let mut died: BTreeSet<ObjectId> = BTreeSet::new();
    for event in events {
        match event {
            Event::DamageDealt {
                source,
                target,
                amount,
            } => {
                *totals.entry((*source, *target)).or_default() += amount;
                match target {
                    DamageTarget::Player(p) => *to_players.entry(*p).or_default() += amount,
                    DamageTarget::Object(o) => *to_objects.entry(*o).or_default() += amount,
                }
            }
            Event::CreatureDied { object, .. } => {
                died.insert(*object);
            }
            _ => {}
        }
    }

    let mut expected: Vec<_> = forecast
        .attackers
        .iter()
        .flat_map(|a| a.deals.iter().chain(&a.receives))
        .filter(|hit| hit.amount > 0)
        .map(|hit| (hit.source, hit.target, hit.amount))
        .collect();
    expected.sort();
    let dealt: Vec<(ObjectId, DamageTarget, u32)> = totals
        .into_iter()
        .map(|((source, target), amount)| (source, target, amount))
        .collect();
    if expected != dealt {
        return Some(format!(
            "damage packets: forecast {expected:?}, real {dealt:?}"
        ));
    }

    for creature in &forecast.creatures {
        let real = to_objects.get(&creature.object).copied().unwrap_or(0);
        if creature.damage != real {
            return Some(format!(
                "{:?} takes {} in the forecast, {real} for real",
                creature.object, creature.damage
            ));
        }
    }
    let known: BTreeSet<_> = forecast.creatures.iter().map(|c| c.object).collect();
    if let Some(stranger) = to_objects.keys().find(|o| !known.contains(o)) {
        return Some(format!(
            "{stranger:?} took damage but is not in the forecast"
        ));
    }
    let doomed: BTreeSet<_> = forecast.dying().collect();
    if doomed != died {
        return Some(format!("deaths: forecast {doomed:?}, real {died:?}"));
    }

    for player in &forecast.players {
        let real = to_players.get(&player.player).copied().unwrap_or(0);
        let life = after
            .player(player.player)
            .map(mtg_engine::player::Player::life);
        if player.damage != real || life != Some(player.life_after) {
            return Some(format!(
                "{:?} takes {} to {} in the forecast, {real} to {life:?} for real",
                player.player, player.damage, player.life_after
            ));
        }
    }
    let attacked: BTreeSet<_> = forecast.players.iter().map(|p| p.player).collect();
    to_players
        .keys()
        .find(|p| !attacked.contains(p))
        .map(|stranger| format!("{stranger:?} took damage but is not in the forecast"))
}
