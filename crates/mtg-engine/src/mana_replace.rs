//! Replacements of the mana a permanent adds as it is tapped for mana (CR 106.6, 614.1): "If a land is tapped for mana, it
//! produces {B} instead of any other type and amount", "If you tap a permanent for mana, it produces twice as much of that
//! mana instead".
//!
//! The replacement changes the options a payment plans with, so the planner and the activation agree on what a tap makes.
//! Two replacements of one tap apply in the order of the rules in force.

use mtg_cards::modify::ManaChange;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;

use crate::mana::ManaOption;
use crate::resolve::Scope;
use crate::state::GameState;

/// The most mana one tap adds after its replacements: a multiplier chain cannot run away.
const MOST: usize = 64;

/// `options` with the replacements in force applied, equal options kept once (a variant of an ability is a source of its own and stays).
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
    for mut option in options {
        for rule in rules.iter().filter(|rule| rule.who.contains(&player)) {
            let Some(source) = state.object(rule.source.id) else {
                continue;
            };
            let scope = Scope::announcing(state, rule.controller, source.reference());
            let Some(tapped) = state.object(option.source.object) else {
                continue;
            };
            if scope
                .selects(&rule.what, tapped.reference())
                .unwrap_or(false)
            {
                change(&mut option, rule.change);
            }
        }
        if !found.contains(&option) {
            found.push(option);
        }
    }
    found
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
