//! How a bot plays the Delves Spirit rule (`docs/delves.md`, Spirits): a fallen seat spends its ectoplasm on haunt
//! actions at the moments they matter and rises again once it can; a living seat revives a fallen teammate when it
//! can spare the life. A plain policy, so a Spirit never costs a search.

use mtg_cards::ability::Ability;
use mtg_cards::cost::CostItem;
use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;
use mtg_engine::abilities::AbilityRef;
use mtg_engine::decision::{PriorityAction, PriorityOptions};
use mtg_engine::spirits::ectoplasm;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameState};

use super::seen::Seen;

/// What a haunt action does, read from its effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Haunt {
    /// Returns a Spirit to life.
    Rise,
    /// Prevents damage to the team.
    Ward,
    /// Lets teammates scry.
    Whisper,
    /// Taps opposing creatures; `big` for one that taps more than one.
    Hinder {
        /// It may tap more than one creature.
        big: bool,
    },
    /// Drains an opponent.
    Wail,
}

/// The kind of the ability `rules`, if it is a haunt action or a revival.
pub fn kind(rules: &Ability) -> Option<Haunt> {
    let Ability::Activated {
        effects, targets, ..
    } = rules
    else {
        return None;
    };
    let has = |pick: fn(&Effect) -> bool| effects.iter().any(pick);
    if has(|e| matches!(e, Effect::Revive { .. })) {
        Some(Haunt::Rise)
    } else if has(|e| matches!(e, Effect::Shield { .. })) {
        Some(Haunt::Ward)
    } else if has(|e| matches!(e, Effect::Scry { .. })) {
        Some(Haunt::Whisper)
    } else if has(|e| matches!(e, Effect::Tap { .. })) {
        let big = targets.iter().any(|t| {
            t.bounds
                .max
                .as_ref()
                .and_then(mtg_cards::value::Amount::fixed)
                .is_some_and(|n| n > 1)
        });
        Some(Haunt::Hinder { big })
    } else if has(|e| matches!(e, Effect::LoseLife { .. })) {
        Some(Haunt::Wail)
    } else {
        None
    }
}

/// The ectoplasm an ability spends: the counters its cost takes from its controller.
fn price(rules: &Ability) -> u32 {
    let Ability::Activated { cost, .. } = rules else {
        return 0;
    };
    cost.0
        .iter()
        .map(|item| match item {
            CostItem::SpendCounters { counter, count } if *counter == ectoplasm() => {
                count.fixed().unwrap_or(0)
            }
            _ => 0,
        })
        .sum()
}

/// The abilities offered now on the emblems in `seat`'s command zone, with their kind and price.
fn offered(
    state: &GameState,
    seat: PlayerId,
    options: &PriorityOptions,
) -> Vec<(AbilityRef, Haunt, u32)> {
    options
        .abilities
        .iter()
        .filter_map(|ability| {
            let object = state.object(ability.object)?;
            if object.zone() != Zone::Command(seat) {
                return None;
            }
            let rules = object.card().rules.get(ability.ability)?;
            Some((*ability, kind(rules)?, price(rules)))
        })
        .collect()
}

/// The ectoplasm `seat` holds.
fn ectoplasm_of(state: &GameState, seat: PlayerId) -> u32 {
    state
        .player(seat)
        .map_or(0, |p| p.counters_of(&ectoplasm()))
}

/// The power attacking `seat`'s team now.
fn incoming(state: &GameState, seen: &Seen<'_>) -> i32 {
    state
        .combat()
        .attackers()
        .iter()
        .filter(|a| !a.blocked() && a.defender(state).is_some_and(|p| seen.is_teammate(p)))
        .filter_map(|a| seen.body(a.object()))
        .map(|body| body.power().max(0))
        .sum()
}

/// The strongest untapped creature of an opponent.
fn threat(seen: &Seen<'_>) -> i32 {
    seen.bodies()
        .iter()
        .filter(|b| b.is_creature() && !b.tapped && !seen.is_teammate(b.controller))
        .map(super::seen::Body::power)
        .max()
        .unwrap_or(0)
}

/// The play of `seat` at priority under the Spirit rule: `Some` for a Spirit (a haunt action or a pass) and for a
/// living seat that revives a teammate; `None` leaves the decision to the brain.
pub fn plan(engine: &Engine, seat: PlayerId, options: &PriorityOptions) -> Option<PriorityAction> {
    let state = engine.state();
    let player = state.player(seat)?;
    let seen = Seen::new(state, seat);
    if !player.is_spirit() {
        let fallen = state
            .players()
            .iter()
            .any(|p| p.id() != seat && p.is_spirit() && p.is_in_game() && seen.is_teammate(p.id()));
        return offered(state, seat, options)
            .into_iter()
            .find(|(_, kind, _)| *kind == Haunt::Rise)
            .filter(|_| fallen && player.life() >= 14)
            .map(|(ability, ..)| PriorityAction::Activate(ability));
    }
    if std::env::var_os("DELVE_SIM_IDLE_SPIRITS").is_some() {
        return Some(PriorityAction::Pass);
    }
    let actions = offered(state, seat, options);
    let find = |want: fn(Haunt) -> bool| {
        actions
            .iter()
            .find(|(_, kind, _)| want(*kind))
            .map(|(ability, ..)| PriorityAction::Activate(*ability))
    };
    if let Some(rise) = find(|k| k == Haunt::Rise) {
        return Some(rise);
    }
    let ecto = ectoplasm_of(state, seat);
    let turn = state.turn();
    let theirs = !seen.is_teammate(turn.active());
    let empty = seen.stack().is_empty();
    // Attackers at the team: shield it when the hit is worth an ectoplasm.
    if theirs && empty && turn.step() == Step::DeclareAttackers && incoming(state, &seen) >= 4 {
        return Some(find(|k| k == Haunt::Ward).unwrap_or(PriorityAction::Pass));
    }
    // Before the opponents' combat: tap their biggest attackers, keeping some ectoplasm toward rising.
    if theirs && empty && turn.step() == Step::BeginCombat {
        if ecto >= 4
            && let Some(howl) = find(|k| k == Haunt::Hinder { big: true })
        {
            return Some(howl);
        }
        if ecto >= 2 && threat(&seen) >= 4 {
            return Some(
                find(|k| k == Haunt::Hinder { big: false }).unwrap_or(PriorityAction::Pass),
            );
        }
    }
    Some(PriorityAction::Pass)
}
