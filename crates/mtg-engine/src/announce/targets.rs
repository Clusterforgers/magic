//! The targets of each target specification (CR 601.2c, 115).
//!
//! A specification that allows no targets, or has none to offer and needs none, is passed over; one that cannot be met
//! rewinds the announcement.

use mtg_cards::refs::TargetSpec;
use mtg_cards::value::CastWay;

use super::{Ctx, Next, What, Work};
use crate::attach;
use crate::choose::{Choices, announced_specs, asks, options_for};
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
        ) else {
            return Some(Next::Drop);
        };
        if !asks(&options) {
            work.choices.targets.push(Vec::new());
            continue;
        }
        options.triggered = ctx.triggered.is_some();
        return Some(ctx.ask(DecisionKind::ChooseTargets(options)));
    }
    None
}
