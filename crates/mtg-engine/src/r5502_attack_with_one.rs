//! "Each opponent must attack with at least one creature each combat if able." and "... must attack you or a
//! planeswalker you control with at least one creature ..." (`GameRule::MustAttackWithOne`, CR 508.1d): a requirement
//! on the whole declaration rather than on one creature. Each rule in force that names the attacking player is a group
//! of defenders (any defender when empty); a group is listed while some creature offered can attack one of its defenders
//! for nothing, alone, and the declaration names an attacker at each listed group.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::combat::AttackTarget;
use crate::decision::{AttackOptions, AttackerOption};
use crate::state::GameState;

/// Whether `target` belongs to `group` (an empty group is any defender).
fn r5502_in(group: &[AttackTarget], target: AttackTarget) -> bool {
    group.is_empty() || group.contains(&target)
}

/// The defenders of `group` the creature of `option` may attack for nothing.
fn r5502_free_in(option: &AttackerOption, group: &[AttackTarget]) -> Option<AttackTarget> {
    option.defenders.iter().copied().find(|target| {
        r5502_in(group, *target)
            && !option.tax.iter().any(|(taxed, _)| taxed == target)
            && !option.life.iter().any(|(taxed, _)| taxed == target)
    })
}

/// The groups of defenders `player` must attack with at least one creature, each one some creature of `options` that
/// can attack alone and is bound by no join can attack for nothing; each group once.
pub(crate) fn r5502_groups(
    state: &GameState,
    player: PlayerId,
    options: &AttackOptions,
) -> Vec<Vec<AttackTarget>> {
    let mut groups: Vec<Vec<AttackTarget>> = Vec::new();
    for (players, at) in &state.rules().attack_with_one {
        if !players.contains(&player) || groups.contains(at) {
            continue;
        }
        let able = options.attackers.iter().any(|option| {
            !options.cant_alone.contains(&option.object)
                && !crate::r286_joins::r286_bound(&options.joins, option.object)
                && r5502_free_in(option, at).is_some()
        });
        if able {
            groups.push(at.clone());
        }
    }
    groups
}

impl AttackOptions {
    /// Whether the declaration leaves a listed group with no attacker.
    pub(crate) fn r5502_unmet(&self, attack: &[(ObjectId, AttackTarget)]) -> bool {
        self.at_least_one
            .iter()
            .flat_map(|groups| groups.iter())
            .any(|group| !attack.iter().any(|(_, target)| r5502_in(group, *target)))
    }

    /// `attack` with a creature added for each listed group it leaves with no attacker: the first creature not yet in it
    /// that can attack alone and may attack one of the group's defenders for nothing, at that defender.
    pub(crate) fn r5502_with_one(
        &self,
        mut attack: Vec<(ObjectId, AttackTarget)>,
    ) -> Vec<(ObjectId, AttackTarget)> {
        for group in self.at_least_one.iter().flat_map(|groups| groups.iter()) {
            if attack.iter().any(|(_, target)| r5502_in(group, *target)) {
                continue;
            }
            let added = self.attackers.iter().find_map(|option| {
                let free = !attack.iter().any(|(object, _)| *object == option.object)
                    && !self.cant_alone.contains(&option.object)
                    && !crate::r286_joins::r286_bound(&self.joins, option.object);
                free.then(|| r5502_free_in(option, group))
                    .flatten()
                    .map(|target| (option.object, target))
            });
            attack.extend(added);
        }
        attack
    }
}
