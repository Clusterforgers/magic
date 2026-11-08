//! Replacements of the mana a permanent adds as it is tapped for mana (CR 106.6, 614.1): "If a land is tapped for mana, it
//! produces {B} instead of any other type and amount", "If you tap a permanent for mana, it produces twice as much of that
//! mana instead".
//!
//! The replacement changes the options a payment plans with, so the planner and the activation agree on what a tap makes.
//! The player whose tap it is chooses the order of the replacements that apply to it (CR 616.1): each order that makes
//! different mana is a source of its own. An ability that does not tap its source is no tap for mana and stays as it is.

use mtg_cards::modify::ManaChange;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;

use crate::mana::ManaOption;
use crate::resolve::Scope;
use crate::rules::ManaRule;
use crate::state::GameState;

/// The most mana one tap adds after its replacements: a multiplier chain cannot run away.
const MOST: usize = 64;

/// The distance between the source numbers of the variants the orders of the replacements make of one option; a
/// multiple of `mana_bonus::STRIDE`, so the activation still finds the ability it stands for.
const ORDER_STRIDE: usize = 1 << 28;

/// The most replacements whose orders are tried; more apply in the order of the rules.
const MOST_ORDERED: usize = 4;

/// `options` with the replacements in force applied, equal options kept once (a variant of an ability is a source of
/// its own and stays); the order of the rules first.
pub(crate) fn fold(
    state: &GameState,
    player: PlayerId,
    options: Vec<ManaOption>,
) -> Vec<ManaOption> {
    let rules = &state.rules().mana;
    if rules.is_empty() {
        return options;
    }
    let mut found: Vec<ManaOption> = Vec::with_capacity(options.len());
    for option in options {
        for made in orders(state, player, rules, option) {
            if !found.contains(&made) {
                found.push(made);
            }
        }
    }
    found
}

/// The options one option becomes, one for each order of the replacements that makes different mana.
fn orders(
    state: &GameState,
    player: PlayerId,
    rules: &[ManaRule],
    option: ManaOption,
) -> Vec<ManaOption> {
    if option.tapless {
        return vec![option];
    }
    let Some(tapped) = state.object(option.source.object) else {
        return vec![option];
    };
    let changes: Vec<ManaChange> = rules
        .iter()
        .filter(|rule| rule.who.contains(&player))
        .filter(|rule| {
            state.object(rule.source.id).is_some_and(|source| {
                Scope::announcing(state, rule.controller, source.reference())
                    .selects(&rule.what, tapped.reference())
                    .unwrap_or(false)
            })
        })
        .map(|rule| rule.change)
        .collect();
    let ordered = changes.len().min(MOST_ORDERED);
    let mut found: Vec<ManaOption> = Vec::new();
    for order in permutations(ordered) {
        let mut next = option.clone();
        for at in order {
            change(&mut next, changes[at]);
        }
        // Replacements past the limit apply last, in the order of the rules.
        for &how in &changes[ordered..] {
            change(&mut next, how);
        }
        let same = |kept: &ManaOption| {
            (kept.produces, kept.count, &kept.also) == (next.produces, next.count, &next.also)
        };
        if !found.iter().any(same) {
            next.source.ability += found.len() * ORDER_STRIDE;
            found.push(next);
        }
    }
    found
}

/// Every order of `0..n`, the rule order first.
fn permutations(n: usize) -> Vec<Vec<usize>> {
    if n == 0 {
        return vec![Vec::new()];
    }
    let mut all = Vec::new();
    for first in 0..n {
        for mut tail in permutations(n - 1) {
            for at in &mut tail {
                if *at >= first {
                    *at += 1;
                }
            }
            tail.insert(0, first);
            all.push(tail);
        }
    }
    all
}

/// The option after one replacement.
fn change(option: &mut ManaOption, how: ManaChange) {
    match how {
        ManaChange::OnlyType(kind) => {
            option.produces = kind;
            option.also.iter_mut().for_each(|mana| *mana = kind);
        }
        ManaChange::One(kind) => {
            option.produces = kind;
            option.count = 1;
            option.also.clear();
        }
        ManaChange::Times(times) => {
            let times = usize::try_from(times).unwrap_or(1).max(1);
            let count = usize::try_from(option.count).unwrap_or(MOST);
            option.count = u32::try_from((count * times).min(MOST)).unwrap_or(1);
            let also: Vec<ManaType> = option.also.repeat(times);
            option.also = also.into_iter().take(MOST).collect();
        }
        ManaChange::Convert { from, to } => {
            if option.produces == from {
                option.produces = to;
            }
            option
                .also
                .iter_mut()
                .filter(|mana| **mana == from)
                .for_each(|mana| *mana = to);
        }
    }
}

#[cfg(test)]
mod tests;
