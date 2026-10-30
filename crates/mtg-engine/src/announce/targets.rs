//! The targets of each target specification (CR 601.2c, 115).
//!
//! A specification that allows no targets, or has none to offer and needs none, is passed over; one that cannot be met
//! rewinds the announcement.

use mtg_cards::refs::TargetSpec;
use mtg_cards::value::CastWay;

use super::{Ctx, Next, What, Work, total};
use crate::attach;
use crate::choose::{Choices, Target, announced_specs, asks, options_for, without_earlier};
use crate::decision::DecisionKind;

/// The target specifications announced, in the order they are asked: an Aura spell's one, from its enchant ability
/// (CR 303.4a), otherwise those of the rules or, for a modal ability, of the modes chosen.
pub(super) fn specs(ctx: &Ctx<'_>, choices: &Choices) -> Vec<TargetSpec> {
    // An overloaded spell has no targets: each object that could be one is affected (CR 702.96).
    if choices.way == Some(CastWay::Overload) {
        return Vec::new();
    }
    // A creature card cast for its bestow cost is an Aura spell with enchant creature (CR 702.103b).
    if choices.way == Some(CastWay::Bestow) && matches!(ctx.what, What::Spell(_)) {
        return vec![attach::bestow_target()];
    }
    // A creature card cast for its mutate cost targets a non-Human creature with the same owner (CR 702.140a).
    if choices.way == Some(CastWay::Mutate) && matches!(ctx.what, What::Spell(_)) {
        return vec![crate::merged::target_spec()];
    }
    let modes = &choices.modes;
    if let What::Spell(spell) = ctx.what
        && let Some(spec) = attach::spell_target(ctx.state, *spell)
    {
        return vec![spec];
    }
    ctx.ability.map_or_else(Vec::new, |ability| {
        announced_specs(ability, modes)
            .into_iter()
            .cloned()
            .collect()
    })
}

/// Leaves out of `legal`, the targets offered for the next of a spell's `specs` target specifications, those with
/// which its cost can no longer be paid: a reduction that reads the targets ("spells that target a creature cost {2}
/// less") is counted on before the targets are chosen, so only the targets that bring it are offered. Nothing is left
/// out while the cost can be paid whatever is chosen.
fn payable_targets(ctx: &Ctx<'_>, choices: &Choices, specs: usize, legal: &mut Vec<Target>) {
    if !matches!(ctx.what, What::Spell(_)) || !ctx.state.possible.costs {
        return;
    }
    let payable = |targets: Vec<Vec<Target>>| {
        let trial = Choices {
            targets,
            ..choices.clone()
        };
        total::of(ctx, &trial).is_some_and(|total| total::affordable(ctx, &total))
    };
    let mut none = choices.targets.clone();
    none.resize(specs, Vec::new());
    if payable(none) {
        return;
    }
    legal.retain(|target| {
        let mut chosen = choices.targets.clone();
        chosen.push(vec![*target]);
        payable(chosen)
    });
}

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    let specs = specs(ctx, work.choices);
    while work.choices.targets.len() < specs.len() {
        let index = work.choices.targets.len();
        let Some(mut options) = options_for(
            ctx.state,
            ctx.source,
            ctx.player,
            index,
            &specs[index],
            ctx.can_cancel(),
            ctx.cause(),
            work.choices.x,
            &work.choices.optional_costs,
            work.choices.way,
        ) else {
            return Some(Next::Drop);
        };
        // "Another target" cannot name what an earlier specification took: the minimum may no longer be met.
        without_earlier(&specs[index], &work.choices.targets, &mut options.legal);
        payable_targets(ctx, work.choices, specs.len(), &mut options.legal);
        let left = u32::try_from(options.legal.len()).unwrap_or(u32::MAX);
        if left < options.min {
            return Some(Next::Drop);
        }
        options.max = options.max.min(left);
        if !asks(&options) {
            work.choices.targets.push(Vec::new());
            continue;
        }
        options.triggered = ctx.triggered.is_some();
        return Some(ctx.ask(DecisionKind::ChooseTargets(options)));
    }
    None
}
