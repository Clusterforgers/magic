//! Combat damage: dividing an attacker's damage among its blockers and the player it attacks, and dealing it
//! simultaneously in the first-strike step and the regular one.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::characteristics::characteristics;
use crate::combat::{AttackTarget, Attacker, DamageTarget};
use crate::decision::DamageOptions;
use crate::error::IllegalAnswer;
use crate::events::Event;
use crate::keywords::{self, DamageRules};
use crate::object::GameObject;
use crate::state::GameState;
use crate::turn::Step;
use crate::zones::Zone;

/// Which combat damage step is being dealt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DamagePass {
    /// The first-strike step (creatures with first strike or double strike).
    FirstStrike,
    /// The regular step.
    Regular,
}

/// Whether a first-strike damage step occurs this combat: an attacker or a blocker has first strike or double strike (CR 510.4).
pub(crate) fn first_strike_step_needed(state: &GameState) -> bool {
    state.combat().attackers().iter().any(|attacker| {
        std::iter::once(attacker.object())
            .chain(attacker.blockers().iter().copied())
            .any(|id| keywords::damage_rules(state, id).strikes_first())
    })
}

/// The damage steps still ahead in the combat in progress, in order.
pub(crate) fn passes_ahead(state: &GameState) -> &'static [DamagePass] {
    const BOTH: [DamagePass; 2] = [DamagePass::FirstStrike, DamagePass::Regular];
    const REGULAR: [DamagePass; 1] = [DamagePass::Regular];
    let first_strike_over = match state.turn().step() {
        Step::FirstStrikeDamage => state.priority().is_some(),
        Step::CombatDamage => true,
        _ => !first_strike_step_needed(state),
    };
    if first_strike_over { &REGULAR } else { &BOTH }
}

/// Damage `id` deals in `pass`; `None` if it is not on the battlefield or does not strike in this pass.
///
/// A creature with first strike alone strikes in the first-strike step only, one with double strike in both,
/// and every other creature in the regular step (CR 510.4).
pub(crate) fn strike_power(state: &GameState, id: ObjectId, pass: DamagePass) -> Option<u32> {
    let on_battlefield = state
        .object(id)
        .is_some_and(|o| o.zone() == Zone::Battlefield);
    if !on_battlefield {
        return None;
    }
    let rules = keywords::damage_rules(state, id);
    let strikes = match pass {
        DamagePass::FirstStrike => rules.strikes_first(),
        DamagePass::Regular => rules.strikes_regularly(),
    };
    if !strikes {
        return None;
    }
    let power = characteristics(state, id)?.power.unwrap_or(0).max(0);
    Some(u32::try_from(power).unwrap_or(0))
}

/// Damage that is lethal to `id` from a source that does or does not have deathtouch (CR 120.6, 702.2c).
///
/// It is what the toughness leaves after the damage marked, and 1 at most from deathtouch: any damage from such
/// a source is lethal, but a creature already holding lethal damage needs none.
pub(crate) fn lethal_damage(state: &GameState, id: ObjectId, deathtouch: bool) -> u32 {
    let toughness = characteristics(state, id)
        .and_then(|c| c.toughness)
        .unwrap_or(0);
    let marked = state
        .object(id)
        .and_then(GameObject::permanent)
        .map_or(0, crate::object::PermanentState::damage);
    let left = u32::try_from((i64::from(toughness) - i64::from(marked)).max(0)).unwrap_or(u32::MAX);
    if deathtouch { left.min(1) } else { left }
}

/// What an attacker's damage amounts to in one step: dealt as it stands or waiting for its controller's division.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Division {
    /// Nothing is left to choose: these shares are dealt, in order.
    Fixed(Vec<(DamageTarget, u32)>),
    /// The attacker's controller divides the damage.
    Choose(DamageOptions),
}

/// What the attacker attacks, as a recipient of damage.
fn defender_of(attacker: &Attacker) -> DamageTarget {
    match attacker.target() {
        AttackTarget::Player(p) => DamageTarget::Player(p),
        AttackTarget::Planeswalker(o) => DamageTarget::Object(o),
    }
}

/// How the attacker's `power` is shared among its blockers and the player or planeswalker it attacks.
///
/// An unblocked attacker deals all of it to what it attacks. A blocked one deals it to its blockers: all to
/// the only one, as its controller divided it among several, or as the default division has it when the
/// controller has not divided yet and has a choice to make. Trample (CR 702.19) adds what it attacks as a
/// recipient of the damage that goes beyond lethal to every blocker, and of all of it when no blocker is left.
pub(crate) fn division(
    state: &GameState,
    attacker: &Attacker,
    power: u32,
    rules: DamageRules,
) -> Division {
    let defender = defender_of(attacker);
    if !attacker.blocked() {
        return Division::Fixed(vec![(defender, power)]);
    }
    let blockers = attacker.blockers();
    if blockers.is_empty() {
        return Division::Fixed(if rules.trample {
            vec![(defender, power)]
        } else {
            Vec::new()
        });
    }
    if let Some(recorded) = &attacker.assignment {
        let mut shares: Vec<(DamageTarget, u32)> = blockers
            .iter()
            .map(|blocker| {
                let share = recorded
                    .iter()
                    .filter(|(b, _)| b == blocker)
                    .fold(0u32, |sum, (_, n)| sum.saturating_add(*n));
                (DamageTarget::Object(*blocker), share)
            })
            .collect();
        let assigned = shares
            .iter()
            .fold(0u32, |sum, (_, n)| sum.saturating_add(*n));
        if rules.trample && power > assigned {
            shares.push((defender, power - assigned));
        }
        return Division::Fixed(shares);
    }
    let options = DamageOptions {
        attacker: attacker.object(),
        power,
        blockers: blockers.to_vec(),
        excess: rules.trample.then_some(defender),
        lethal: blockers
            .iter()
            .map(|blocker| lethal_damage(state, *blocker, rules.deathtouch))
            .collect(),
    };
    if has_choice(&options) {
        Division::Choose(options)
    } else {
        Division::Fixed(default_shares(&options))
    }
}

/// Whether the attacker's controller has more than one way to divide the damage.
fn has_choice(options: &DamageOptions) -> bool {
    if options.power == 0 {
        return false;
    }
    match (options.excess.is_some(), options.blockers.as_slice()) {
        (_, []) | (false, [_]) => false,
        (true, [_]) => options
            .lethal
            .first()
            .is_some_and(|lethal| options.power > *lethal),
        _ => true,
    }
}

/// The division a controller most often wants: each blocker in order gets lethal damage while power lasts,
/// and what is left goes to the player or planeswalker attacked with trample and else onto the last blocker.
pub(crate) fn default_shares(options: &DamageOptions) -> Vec<(DamageTarget, u32)> {
    let mut left = options.power;
    let mut shares = Vec::with_capacity(options.blockers.len() + 1);
    for (at, blocker) in options.blockers.iter().enumerate() {
        let last = at + 1 == options.blockers.len();
        let lethal = options.lethal.get(at).copied().unwrap_or(0);
        let share = if last && options.excess.is_none() {
            left
        } else {
            left.min(lethal)
        };
        left -= share;
        shares.push((DamageTarget::Object(*blocker), share));
    }
    if let Some(excess) = options.excess {
        shares.push((excess, left));
    }
    shares
}

/// The first attacker whose controller must divide damage in `pass`, and the choice it faces.
pub(crate) fn pending_assignment(
    state: &GameState,
    pass: DamagePass,
) -> Option<(PlayerId, DamageOptions)> {
    state.combat().attackers().iter().find_map(|attacker| {
        let power = strike_power(state, attacker.object(), pass)?;
        let rules = keywords::damage_rules(state, attacker.object());
        match division(state, attacker, power, rules) {
            Division::Choose(options) => {
                let controller = state.object(attacker.object())?.controller();
                Some((controller, options))
            }
            Division::Fixed(_) => None,
        }
    })
}

/// Validates how an attacker divided its damage among its blockers and records it.
///
/// With trample the damage not given to a blocker goes to the player or planeswalker attacked, which needs
/// lethal damage on every blocker first (CR 702.19b). Without it all the damage goes to the blockers.
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
    let assigned = assignment
        .iter()
        .fold(0u32, |sum, (_, n)| sum.saturating_add(*n));
    let split_error = IllegalAnswer::DamageSplit {
        power: options.power,
        assigned,
    };
    if options.excess.is_none() {
        if assigned != options.power {
            return Err(split_error);
        }
    } else if assigned > options.power {
        return Err(split_error);
    } else if assigned < options.power {
        let lethal_everywhere = options
            .blockers
            .iter()
            .zip(&options.lethal)
            .all(|(b, lethal)| {
                assignment
                    .iter()
                    .find(|(given, _)| given == b)
                    .map_or(0, |(_, n)| *n)
                    >= *lethal
            });
        if !lethal_everywhere {
            return Err(IllegalAnswer::ExcessBeforeLethal);
        }
    }
    Ok(Outcome::Events(vec![Event::DamageAssigned {
        attacker: options.attacker,
        assignment,
    }]))
}

/// The events of `source` dealing `amount` damage to `target`: the damage, then the life its controller gains
/// when it has lifelink (CR 702.15), in the same batch. Damage of nothing is no event.
pub(crate) fn dealt(
    state: &GameState,
    source: ObjectId,
    target: DamageTarget,
    amount: u32,
) -> Vec<Event> {
    if amount == 0 {
        return Vec::new();
    }
    let mut events = vec![Event::DamageDealt {
        source,
        target,
        amount,
    }];
    let controller = state
        .object(source)
        .map(GameObject::controller)
        .filter(|controller| state.is_alive(*controller));
    if let Some(player) = controller
        && keywords::damage_rules(state, source).lifelink
    {
        events.push(Event::LifeGained { player, amount });
    }
    events
}

/// All combat damage for `pass`, dealt simultaneously from the same pre-damage state.
///
/// An attacker whose controller has not divided its damage yet deals the default division, which is what
/// `forecast::combat_forecast` runs this function for: the real step asks the controllers first.
pub(crate) fn damage_events(state: &GameState, pass: DamagePass) -> Vec<Event> {
    let mut events = Vec::new();
    for attacker in state.combat().attackers() {
        if let Some(power) = strike_power(state, attacker.object(), pass) {
            let rules = keywords::damage_rules(state, attacker.object());
            let shares = match division(state, attacker, power, rules) {
                Division::Fixed(shares) => shares,
                Division::Choose(options) => default_shares(&options),
            };
            for (target, amount) in shares {
                events.extend(dealt(state, attacker.object(), target, amount));
            }
        }
        for &blocker in attacker.blockers() {
            if let Some(power) = strike_power(state, blocker, pass) {
                events.extend(dealt(
                    state,
                    blocker,
                    DamageTarget::Object(attacker.object()),
                    power,
                ));
            }
        }
    }
    events
}
