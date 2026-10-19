//! The combat forecast: what combat damage would do if it were dealt right now.
//!
//! The forecast is derived data, recomputed from the state whenever it is asked for and never stored. It
//! plays the damage steps still ahead on a copy of the state with the engine's own code: `damage_events`
//! deals a step, the events fold, and state-based actions settle before the next step, so keywords (first
//! strike, deathtouch, trample, lifelink, indestructible) are in the forecast because they are in the
//! damage. An attacker whose controller has not divided its damage yet deals the default division.

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::characteristics::{Characteristics, characteristics};
use crate::combat::{AttackTarget, Attacker, DamageTarget};
use crate::damage::{DamagePass, damage_events, passes_ahead};
use crate::events::Event;
use crate::fold::apply_event;
use crate::keywords;
use crate::sba;
use crate::state::{GameState, Stage};
use crate::turn::Step;

/// One expected packet of combat damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ForecastDamage {
    /// The creature that deals it.
    pub source: ObjectId,
    /// Who or what receives it.
    pub target: DamageTarget,
    /// How much in all the damage steps ahead (may be 0 for a creature with no power or a blocker left without a share).
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
    /// Damage it assigns: one packet per blocker in declared order, or one to the defender when unblocked, and
    /// with trample one to the defender after the blockers. A creature that strikes twice has both in one packet.
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
    /// Whether state-based actions destroy it once the damage is dealt: lethal damage or deathtouch, unless it is indestructible.
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
    /// Their life total after the damage and the life it gains (lifelink).
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
/// attacker is left and after the last damage has been dealt.
///
/// After the damage step has dealt its damage the combat state still exists until the end of combat,
/// so this is the function a view uses; [`combat_forecast`] alone would count the damage twice. While
/// the first-strike step holds priority its damage is done and the regular step is still ahead.
pub fn pending_combat_forecast(state: &GameState) -> Option<CombatForecast> {
    let combat = state.combat();
    if state.stage() != Stage::Turns
        || !combat.attackers_declared()
        || combat.attackers().is_empty()
    {
        return None;
    }
    let pending = match state.turn().step() {
        Step::DeclareAttackers | Step::DeclareBlockers | Step::FirstStrikeDamage => true,
        // A damage step that has not granted priority yet is waiting on a damage assignment.
        Step::CombatDamage => state.priority().is_none(),
        _ => false,
    };
    pending.then(|| combat_forecast(state))
}

/// What combat damage would do if dealt now, from the declared attackers and blockers.
///
/// Meaningful while an attack is declared and damage has not been dealt; see
/// [`pending_combat_forecast`] for the guarded version.
pub fn combat_forecast(state: &GameState) -> CombatForecast {
    let run = Run::of(state);
    let attackers: Vec<AttackerForecast> = state
        .combat()
        .attackers()
        .iter()
        .map(|attacker| attacker_forecast(state, attacker, &run))
        .collect();

    let mut involved: BTreeSet<ObjectId> = BTreeSet::new();
    let mut attacked: BTreeSet<PlayerId> = BTreeSet::new();
    for attacker in state.combat().attackers() {
        involved.insert(attacker.object());
        involved.extend(attacker.blockers());
        if let AttackTarget::Player(player) = attacker.target() {
            attacked.insert(player);
        }
    }
    for (_, target) in run.dealt.keys() {
        match *target {
            DamageTarget::Object(object) => {
                involved.insert(object);
            }
            DamageTarget::Player(player) => {
                attacked.insert(player);
            }
        }
    }

    CombatForecast {
        attackers,
        creatures: involved
            .into_iter()
            .filter_map(|id| creature_forecast(state, id, &run))
            .collect(),
        players: attacked
            .into_iter()
            .filter_map(|id| player_forecast(state, id, &run))
            .collect(),
    }
}

/// The damage steps still ahead played out on a copy of the state.
struct Run {
    /// The copy after the last step and the state-based actions that follow it.
    after: GameState,
    /// Damage dealt over all the steps, by source and recipient.
    dealt: BTreeMap<(ObjectId, DamageTarget), u32>,
    /// Creatures destroyed or put into the graveyard by state-based actions along the way.
    died: BTreeSet<ObjectId>,
}

/// State-based actions that follow a step repeat until none is left; this bounds a bug, not a game.
const MAX_SETTLE_ROUNDS: usize = 64;

impl Run {
    fn of(state: &GameState) -> Run {
        let mut run = Run {
            after: state.clone(),
            dealt: BTreeMap::new(),
            died: BTreeSet::new(),
        };
        for &pass in passes_ahead(state) {
            let step = match pass {
                DamagePass::FirstStrike => Step::FirstStrikeDamage,
                DamagePass::Regular => Step::CombatDamage,
            };
            // Entering a step is what forgets the last step's divisions of damage; a step under way keeps its own.
            let entered = run.after.turn().step() == step || run.fold(&[Event::StepBegan { step }]);
            let events = damage_events(&run.after, pass);
            if !entered || !run.fold(&events) || !run.settle() {
                break;
            }
        }
        run
    }

    /// Folds `events` onto the copy and notes the damage and deaths among them; `false` if one cannot be folded.
    fn fold(&mut self, events: &[Event]) -> bool {
        for event in events {
            if apply_event(&mut self.after, event).is_err() {
                return false;
            }
            match event {
                Event::DamageDealt {
                    source,
                    target,
                    amount,
                } => {
                    let total = self.dealt.entry((*source, *target)).or_default();
                    *total = total.saturating_add(*amount);
                }
                Event::CreatureDied { object, .. } => {
                    self.died.insert(*object);
                }
                _ => {}
            }
        }
        true
    }

    /// Plays the state-based actions the real game runs between steps; `false` once the game is over.
    fn settle(&mut self) -> bool {
        for _ in 0..MAX_SETTLE_ROUNDS {
            let mut events = sba::check(&self.after);
            if events.is_empty() {
                if let Some(result) = self.after.format().game_result(&self.after) {
                    events = vec![Event::GameEnded { result }];
                } else {
                    events = sba::housekeeping(&self.after);
                }
            }
            if events.is_empty() {
                return true;
            }
            if !self.fold(&events) || self.after.stage() == Stage::Finished {
                return false;
            }
        }
        false
    }

    fn dealt_to(&self, source: ObjectId, target: DamageTarget) -> u32 {
        self.dealt.get(&(source, target)).copied().unwrap_or(0)
    }

    fn total_to(&self, target: DamageTarget) -> u32 {
        self.dealt
            .iter()
            .filter(|((_, to), _)| *to == target)
            .fold(0u32, |sum, (_, amount)| sum.saturating_add(*amount))
    }
}

fn attacker_forecast(state: &GameState, attacker: &Attacker, run: &Run) -> AttackerForecast {
    let source = attacker.object();
    let hit = |source, target| ForecastDamage {
        source,
        target,
        amount: run.dealt_to(source, target),
    };
    let defender = match attacker.target() {
        AttackTarget::Player(p) => DamageTarget::Player(p),
        AttackTarget::Planeswalker(o) => DamageTarget::Object(o),
    };
    let mut deals: Vec<ForecastDamage> = if attacker.blocked() {
        attacker
            .blockers()
            .iter()
            .map(|blocker| hit(source, DamageTarget::Object(*blocker)))
            .collect()
    } else {
        vec![hit(source, defender)]
    };
    // A blocked trampler may also assign to what it attacks, even when no blocker is left.
    if attacker.blocked() && keywords::damage_rules(state, source).trample {
        deals.push(hit(source, defender));
    }
    let receives = attacker
        .blockers()
        .iter()
        .map(|&blocker| hit(blocker, DamageTarget::Object(source)))
        .collect();
    AttackerForecast {
        attacker: source,
        target: attacker.target(),
        blocked: attacker.blocked(),
        blockers: attacker.blockers().to_vec(),
        deals,
        receives,
    }
}

fn creature_forecast(state: &GameState, id: ObjectId, run: &Run) -> Option<CreatureForecast> {
    characteristics(state, id).filter(Characteristics::is_creature)?;
    Some(CreatureForecast {
        object: id,
        damage: run.total_to(DamageTarget::Object(id)),
        dies: run.died.contains(&id),
    })
}

fn player_forecast(state: &GameState, id: PlayerId, run: &Run) -> Option<PlayerForecast> {
    let life = state.player(id)?.life();
    let life_after = run.after.player(id)?.life();
    let damage = run.total_to(DamageTarget::Player(id));
    Some(PlayerForecast {
        player: id,
        damage,
        life,
        life_after,
        lethal: damage > 0 && life_after <= 0,
    })
}
