//! Combat damage: dividing damage among blockers and dealing it simultaneously.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::characteristics::characteristics;
use crate::combat::{AttackTarget, Attacker, DamageTarget};
use crate::decision::DamageOptions;
use crate::error::IllegalAnswer;
use crate::events::Event;
use crate::state::GameState;
use crate::zones::Zone;

/// Which combat damage step is being dealt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DamagePass {
    /// The first-strike step (creatures with first strike or double strike).
    FirstStrike,
    /// The regular step.
    Regular,
}

/// Whether a first-strike damage step occurs this combat (never, until the keywords exist).
pub(crate) fn first_strike_step_needed(_state: &GameState) -> bool {
    false
}

/// Damage `id` deals in `pass`; `None` if it is not on the battlefield or does not strike in this pass.
fn strike_power(state: &GameState, id: ObjectId, pass: DamagePass) -> Option<u32> {
    let on_battlefield = state
        .object(id)
        .is_some_and(|o| o.zone() == Zone::Battlefield);
    if !on_battlefield || pass == DamagePass::FirstStrike {
        return None;
    }
    let power = characteristics(state, id)?.power.unwrap_or(0).max(0);
    Some(u32::try_from(power).unwrap_or(0))
}

/// The first attacker whose controller must divide damage among several blockers.
pub(crate) fn pending_assignment(
    state: &GameState,
    pass: DamagePass,
) -> Option<(PlayerId, DamageOptions)> {
    state.combat().attackers().iter().find_map(|a| {
        let power = strike_power(state, a.object(), pass)?;
        if a.blockers.len() < 2 || power == 0 || a.assignment.is_some() {
            return None;
        }
        let controller = state.object(a.object())?.controller();
        Some((
            controller,
            DamageOptions {
                attacker: a.object(),
                power,
                blockers: a.blockers.clone(),
            },
        ))
    })
}

/// Validates how an attacker divided its damage and records it.
pub(crate) fn assignment_answer(
    options: &DamageOptions,
    assignment: Vec<(ObjectId, u32)>,
) -> Result<Outcome, IllegalAnswer> {
    for (i, (blocker, _)) in assignment.iter().enumerate() {
        if assignment[..i].iter().any(|(b, _)| b == blocker) {
            return Err(IllegalAnswer::Duplicate("blocker"));
        }
        if !options.blockers.contains(blocker) {
            return Err(IllegalAnswer::NotOffered("blocker"));
        }
    }
    let assigned: u32 = assignment.iter().map(|(_, n)| *n).sum();
    if assigned != options.power {
        return Err(IllegalAnswer::DamageSplit {
            power: options.power,
            assigned,
        });
    }
    Ok(Outcome::Events(vec![Event::DamageAssigned {
        attacker: options.attacker,
        assignment,
    }]))
}

fn damage(source: ObjectId, target: DamageTarget, amount: u32) -> Option<Event> {
    (amount > 0).then_some(Event::DamageDealt {
        source,
        target,
        amount,
    })
}

fn attacker_damage(state: &GameState, a: &Attacker, power: u32) -> Vec<Event> {
    let source = a.object();
    if !a.blocked() {
        let target = match a.target() {
            AttackTarget::Player(p) => DamageTarget::Player(p),
            AttackTarget::Planeswalker(o) => DamageTarget::Object(o),
        };
        return damage(source, target, power).into_iter().collect();
    }
    match a.blockers() {
        [] => Vec::new(),
        [only] => damage(source, DamageTarget::Object(*only), power)
            .into_iter()
            .collect(),
        blockers => a
            .assignment
            .iter()
            .flatten()
            .filter(|(b, _)| blockers.contains(b) && state.object(*b).is_some())
            .filter_map(|(b, n)| damage(source, DamageTarget::Object(*b), *n))
            .collect(),
    }
}

/// All combat damage for `pass`, dealt simultaneously from the same pre-damage state.
pub(crate) fn damage_events(state: &GameState, pass: DamagePass) -> Vec<Event> {
    let mut events = Vec::new();
    for attacker in state.combat().attackers() {
        if let Some(power) = strike_power(state, attacker.object(), pass) {
            events.extend(attacker_damage(state, attacker, power));
        }
        for &blocker in attacker.blockers() {
            if let Some(power) = strike_power(state, blocker, pass) {
                events.extend(damage(
                    blocker,
                    DamageTarget::Object(attacker.object()),
                    power,
                ));
            }
        }
    }
    events
}
