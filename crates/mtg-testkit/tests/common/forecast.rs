//! Goal: shared check that a combat forecast equals what the engine then did (this is not a test module itself).

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::combat::DamageTarget;
use mtg_engine::forecast::CombatForecast;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameState};

/// Whether `events` (produced from a state in `step_before`) include the dealing of combat damage.
///
/// Damage and the priority grant that follows it are one batch, so a grant inside the damage step
/// marks the batch even when every creature had 0 power and no `DamageDealt` exists.
pub fn damage_was_dealt(step_before: Step, events: &[Event]) -> bool {
    let mut in_damage_step = step_before == Step::CombatDamage;
    for event in events {
        match event {
            Event::StepBegan { step } => in_damage_step = *step == Step::CombatDamage,
            Event::PriorityGranted { .. } if in_damage_step => return true,
            _ => {}
        }
    }
    false
}

/// Compares `forecast` (taken right before the answer) with the events that answer produced.
///
/// Returns the first difference as text.
pub fn disagreement(
    forecast: &CombatForecast,
    events: &[Event],
    after: &GameState,
) -> Option<String> {
    let mut dealt: Vec<(ObjectId, DamageTarget, u32)> = Vec::new();
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
                dealt.push((*source, *target, *amount));
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
    dealt.sort();
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
