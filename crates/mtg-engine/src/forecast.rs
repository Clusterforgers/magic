//! The combat forecast: what combat damage would do if it were dealt right now.
//!
//! The forecast is derived data, recomputed from the state whenever it is asked for and never
//! stored. It mirrors `damage::damage_events` but divides damage by a default rule where the
//! attacker's controller has not chosen yet. Keywords join in three small functions: `strike_power`
//! in `damage` (first strike, double strike, power changes), [`lethal_damage`] (deathtouch) and
//! [`divide`] (trample, other assignment rules). None of them exists yet.

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::characteristics::{Characteristics, characteristics};
use crate::combat::{AttackTarget, Attacker, DamageTarget};
use crate::damage::{DamagePass, strike_power};
use crate::object::PermanentState;
use crate::state::{GameState, Stage};
use crate::turn::Step;

/// The damage pass the forecast describes; first strike will add a second forecast pass.
const PASS: DamagePass = DamagePass::Regular;

/// One expected packet of combat damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ForecastDamage {
    /// The creature that deals it.
    pub source: ObjectId,
    /// Who or what receives it.
    pub target: DamageTarget,
    /// How much (may be 0 for a creature with no power or a blocker left without a share).
    pub amount: u32,
}

/// One attacking creature, its blockers and the damage it deals and takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AttackerForecast {
    /// The attacking creature.
    pub attacker: ObjectId,
    /// What it attacks.
    pub target: AttackTarget,
    /// Whether it was blocked (it stays blocked even if every blocker leaves).
    pub blocked: bool,
    /// Creatures blocking it, in declared order.
    pub blockers: Vec<ObjectId>,
    /// Damage it assigns: one packet per blocker in declared order, or one to the defender when unblocked.
    pub deals: Vec<ForecastDamage>,
    /// Damage its blockers deal to it, one packet per blocker.
    pub receives: Vec<ForecastDamage>,
}

/// What combat damage does to one creature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CreatureForecast {
    /// The creature (an attacker, a blocker or an attacked creature).
    pub object: ObjectId,
    /// Combat damage it is expected to receive, from all sources together.
    pub damage: u32,
    /// Whether the damage already marked plus this damage reaches its toughness.
    pub dies: bool,
}

/// What combat damage does to one player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlayerForecast {
    /// The player under attack.
    pub player: PlayerId,
    /// Combat damage they are expected to take (0 when every attacker aimed at them is blocked).
    pub damage: u32,
    /// Their life total now.
    pub life: i32,
    /// Their life total after the damage.
    pub life_after: i32,
    /// Whether the damage takes them to 0 life or less.
    pub lethal: bool,
}

/// The expected result of combat damage for the combat in progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CombatForecast {
    /// Every attacker in declared order.
    pub attackers: Vec<AttackerForecast>,
    /// Every creature in combat, by id, with its expected damage and fate.
    pub creatures: Vec<CreatureForecast>,
    /// Every player an attacker is aimed at, by seat.
    pub players: Vec<PlayerForecast>,
}

impl CombatForecast {
    /// The creatures expected to die.
    pub fn dying(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.creatures.iter().filter(|c| c.dies).map(|c| c.object)
    }
}

/// The forecast while combat damage is still to come, or `None` before attackers are declared, once no
/// attacker is left and after damage has been dealt.
///
/// After the damage step has dealt its damage the combat state still exists until the end of combat,
/// so this is the function a view uses; [`combat_forecast`] alone would count the damage twice.
pub fn pending_combat_forecast(state: &GameState) -> Option<CombatForecast> {
    let combat = state.combat();
    if state.stage() != Stage::Turns
        || !combat.attackers_declared()
        || combat.attackers().is_empty()
    {
        return None;
    }
    let pending = match state.turn().step() {
        Step::DeclareAttackers | Step::DeclareBlockers => true,
        // A damage step that has not granted priority yet is waiting on a damage assignment.
        Step::FirstStrikeDamage | Step::CombatDamage => state.priority().is_none(),
        _ => false,
    };
    pending.then(|| combat_forecast(state))
}

/// What combat damage would do if dealt now, from the declared attackers and blockers.
///
/// Meaningful while an attack is declared and damage has not been dealt; see
/// [`pending_combat_forecast`] for the guarded version.
pub fn combat_forecast(state: &GameState) -> CombatForecast {
    let mut to_objects: BTreeMap<ObjectId, u32> = BTreeMap::new();
    let mut to_players: BTreeMap<PlayerId, u32> = BTreeMap::new();
    let mut involved: BTreeSet<ObjectId> = BTreeSet::new();
    let mut attacked: BTreeSet<PlayerId> = BTreeSet::new();
    let mut attackers = Vec::new();
    for attacker in state.combat().attackers() {
        let deals = deals_of(state, attacker);
        let receives = receives_of(state, attacker);
        involved.insert(attacker.object());
        involved.extend(attacker.blockers());
        if let AttackTarget::Player(player) = attacker.target() {
            attacked.insert(player);
        }
        for hit in deals.iter().chain(&receives) {
            match hit.target {
                DamageTarget::Player(p) => add(to_players.entry(p).or_default(), hit.amount),
                DamageTarget::Object(o) => add(to_objects.entry(o).or_default(), hit.amount),
            }
        }
        attackers.push(AttackerForecast {
            attacker: attacker.object(),
            target: attacker.target(),
            blocked: attacker.blocked(),
            blockers: attacker.blockers().to_vec(),
            deals,
            receives,
        });
    }
    involved.extend(to_objects.keys());
    attacked.extend(to_players.keys());
    CombatForecast {
        attackers,
        creatures: involved
            .into_iter()
            .filter_map(|id| {
                creature_forecast(state, id, to_objects.get(&id).copied().unwrap_or(0))
            })
            .collect(),
        players: attacked
            .into_iter()
            .filter_map(|id| player_forecast(state, id, to_players.get(&id).copied().unwrap_or(0)))
            .collect(),
    }
}

fn add(total: &mut u32, amount: u32) {
    *total = total.saturating_add(amount);
}

/// Damage the attacker assigns, using the controller's recorded division when there is one.
fn deals_of(state: &GameState, attacker: &Attacker) -> Vec<ForecastDamage> {
    let Some(power) = strike_power(state, attacker.object(), PASS) else {
        return Vec::new();
    };
    let source = attacker.object();
    let hit = |target, amount| ForecastDamage {
        source,
        target,
        amount,
    };
    if !attacker.blocked() {
        let target = match attacker.target() {
            AttackTarget::Player(p) => DamageTarget::Player(p),
            AttackTarget::Planeswalker(o) => DamageTarget::Object(o),
        };
        return vec![hit(target, power)];
    }
    let blockers = attacker.blockers();
    if let [only] = blockers {
        return vec![hit(DamageTarget::Object(*only), power)];
    }
    let shares = match &attacker.assignment {
        Some(recorded) => blockers
            .iter()
            .map(|b| {
                recorded
                    .iter()
                    .filter(|(r, _)| r == b)
                    .fold(0, |sum, (_, n)| u32::saturating_add(sum, *n))
            })
            .collect(),
        None => divide(state, blockers, power),
    };
    blockers
        .iter()
        .zip(shares)
        .map(|(b, share)| hit(DamageTarget::Object(*b), share))
        .collect()
}

/// Damage the attacker's blockers deal back to it.
fn receives_of(state: &GameState, attacker: &Attacker) -> Vec<ForecastDamage> {
    attacker
        .blockers()
        .iter()
        .filter_map(|&blocker| {
            Some(ForecastDamage {
                source: blocker,
                target: DamageTarget::Object(attacker.object()),
                amount: strike_power(state, blocker, PASS)?,
            })
        })
        .collect()
}

/// Damage that would destroy `id` given what is already marked on it (deathtouch will change this).
fn lethal_damage(state: &GameState, id: ObjectId) -> u32 {
    let toughness = characteristics(state, id)
        .and_then(|c| c.toughness)
        .unwrap_or(0);
    let marked = marked_damage(state, id);
    u32::try_from((i64::from(toughness) - i64::from(marked)).max(0)).unwrap_or(u32::MAX)
}

fn marked_damage(state: &GameState, id: ObjectId) -> u32 {
    state
        .object(id)
        .and_then(|o| o.permanent())
        .map_or(0, PermanentState::damage)
}

/// The default division of `power` among several blockers: lethal damage to each in declared order,
/// the rest on the last (trample and other rules will change this).
fn divide(state: &GameState, blockers: &[ObjectId], power: u32) -> Vec<u32> {
    let lethal: Vec<u32> = blockers.iter().map(|b| lethal_damage(state, *b)).collect();
    divide_in_order(power, &lethal)
}

/// Lethal damage to each entry in turn, then everything left over to the last one.
fn divide_in_order(power: u32, lethal: &[u32]) -> Vec<u32> {
    let mut left = power;
    let mut shares = Vec::with_capacity(lethal.len());
    for (i, needed) in lethal.iter().enumerate() {
        let share = if i + 1 == lethal.len() {
            left
        } else {
            left.min(*needed)
        };
        left -= share;
        shares.push(share);
    }
    shares
}

fn creature_forecast(state: &GameState, id: ObjectId, damage: u32) -> Option<CreatureForecast> {
    let chars = characteristics(state, id).filter(Characteristics::is_creature)?;
    let toughness = i64::from(chars.toughness.unwrap_or(0));
    let total = i64::from(marked_damage(state, id)) + i64::from(damage);
    Some(CreatureForecast {
        object: id,
        damage,
        dies: toughness <= 0 || total >= toughness,
    })
}

fn player_forecast(state: &GameState, id: PlayerId, damage: u32) -> Option<PlayerForecast> {
    let life = state.player(id)?.life();
    let life_after = life.saturating_sub(i32::try_from(damage).unwrap_or(i32::MAX));
    Some(PlayerForecast {
        player: id,
        damage,
        life,
        life_after,
        lethal: damage > 0 && life_after <= 0,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: the default division of damage among blockers gives each lethal damage in order and the rest to the last, for any power and toughness mix.

    use super::divide_in_order;
    use proptest::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case::one_blocker(5, &[3], &[5])]
    #[case::exact_fit(5, &[2, 3], &[2, 3])]
    #[case::leftover_on_last(7, &[2, 3], &[2, 5])]
    #[case::short_of_first(1, &[2, 3], &[1, 0])]
    #[case::short_of_second(4, &[2, 3, 3], &[2, 2, 0])]
    #[case::three_way(9, &[2, 2, 2], &[2, 2, 5])]
    #[case::no_power(0, &[2, 2], &[0, 0])]
    #[case::already_dead_first(3, &[0, 3], &[0, 3])]
    #[case::nobody(3, &[], &[])]
    fn damage_goes_lethal_in_order(
        #[case] power: u32,
        #[case] lethal: &[u32],
        #[case] expected: &[u32],
    ) {
        assert_eq!(divide_in_order(power, lethal), expected);
    }

    proptest! {
        #[test]
        fn division_conserves_power_and_never_overfeeds_early_blockers(
            power in 0u32..40,
            lethal in prop::collection::vec(0u32..10, 1..6),
        ) {
            let shares = divide_in_order(power, &lethal);
            prop_assert_eq!(shares.len(), lethal.len());
            prop_assert_eq!(shares.iter().sum::<u32>(), power);
            let (_, early) = shares.split_last().expect("at least one blocker");
            for (share, needed) in early.iter().zip(&lethal) {
                prop_assert!(share <= needed);
            }
        }
    }
}
