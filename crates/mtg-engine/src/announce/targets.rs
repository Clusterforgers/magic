//! The targets of each target specification (CR 601.2c, 115).
//!
//! A specification that allows no targets, or has none to offer and needs none, is passed over; one that cannot be met
//! rewinds the announcement.

use mtg_cards::refs::{TargetSpec, Targetable};
use mtg_cards::value::CastWay;
use mtg_core::ids::PlayerId;

use super::{Ctx, Next, What, Work, r5509_flagbearer, total};
use crate::attach;
use crate::choose::{
    Choices, Target, announced_specs, asks, options_for, spec_bases, without_earlier,
};
use crate::decision::{Decision, DecisionKind};
use crate::object::{GameObject, ObjectRef};
use crate::resolve::Scope;
use crate::rng::GameRng;

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
        && let Some(spec) = attach::cast_target(ctx.state, *spell)
    {
        return vec![spec];
    }
    // A spell cast for its cleave cost announces the targets of its text without the bracketed words (CR 702.148).
    if let What::Spell(spell) = ctx.what
        && let Some(cleaved) = crate::keywords::cleave::cleaved(ctx.state, *spell, choices.way)
    {
        return announced_specs(&cleaved, modes)
            .into_iter()
            .cloned()
            .collect();
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

/// Leaves out of `legal` the players the earlier specifications took when the ability's modes must each target a
/// different player ("Each mode must target a different player."). Each mode of such an ability has one target.
fn e14c_other_players(ctx: &Ctx<'_>, earlier: &[Vec<Target>], legal: &mut Vec<Target>) {
    let different = ctx
        .ability
        .and_then(crate::choose::modal_of)
        .is_some_and(|modal| modal.different_players);
    if different {
        legal.retain(|target| {
            target.player().is_none() || !earlier.iter().flatten().any(|taken| taken == target)
        });
    }
}

/// For each of the `count` specifications, how many come before the first of its mode; all 0 for specifications that
/// do not come from the ability's modes.
pub(super) fn bases(ctx: &Ctx<'_>, choices: &Choices, count: usize) -> Vec<usize> {
    let from_modes = ctx
        .ability
        .map(|ability| spec_bases(ability, &choices.modes))
        .filter(|bases| bases.len() == count);
    from_modes.unwrap_or_else(|| vec![0; count])
}

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    let specs = specs(ctx, work.choices);
    let bases = bases(ctx, work.choices, specs.len());
    while work.choices.targets.len() < specs.len() {
        let index = work.choices.targets.len();
        let base = bases.get(index).copied().unwrap_or(0).min(index);
        let Some(mut options) = options_for(
            ctx.state,
            ctx.source,
            ctx.player,
            index,
            &specs[index],
            ctx.can_cancel(),
            ctx.cause(),
            work.choices.x,
            work.choices.modes_chooser(),
            &work.choices.optional_costs,
            work.choices.way,
            Some(&work.choices.targets[base..index]),
        ) else {
            return Some(Next::Drop);
        };
        // "Another target" cannot name what an earlier specification took: the minimum may no longer be met.
        without_earlier(&specs[index], &work.choices.targets, &mut options.legal);
        e14c_other_players(ctx, &work.choices.targets[..index], &mut options.legal);
        payable_targets(ctx, work.choices, specs.len(), &mut options.legal);
        if !matches!(specs[index].what, Targetable::AtRandom(_))
            && chooser(ctx, &specs[index]) == ctx.player
        {
            r5509_flagbearer::r5509_narrow(
                ctx,
                work.choices,
                work.done,
                &specs,
                index,
                &mut options,
            );
        }
        let left = u32::try_from(options.legal.len()).unwrap_or(u32::MAX);
        if left < options.min {
            return Some(Next::Drop);
        }
        options.max = options.max.min(left);
        if let Targetable::AtRandom(_) = &specs[index].what {
            pick(ctx, work, options.legal, options.max);
            continue;
        }
        if !asks(&options) {
            let pick = r5509_flagbearer::r5509_finish(work.done);
            work.choices.targets.push(pick.unwrap_or_default());
            continue;
        }
        options.triggered = ctx.triggered.is_some();
        let chooser = chooser(ctx, &specs[index]);
        if chooser != ctx.player {
            // Another player chooses: backing out of the controller's spell or ability is not theirs to do.
            options.can_cancel = false;
            return Some(Next::Ask(Decision {
                player: chooser,
                kind: DecisionKind::ChooseTargets(options),
            }));
        }
        return Some(ctx.ask(DecisionKind::ChooseTargets(options)));
    }
    None
}

/// "Chosen at random": the game draws `count` of the `legal` targets with the next substream of its random source, after
/// those the modes drawn and the earlier random specifications spent. The choices count it, and the event that puts the
/// spell or ability on the stack spends the substream; nothing is drawn when there is nothing to choose from.
fn pick(ctx: &Ctx<'_>, work: &mut Work<'_>, mut legal: Vec<crate::choose::Target>, count: u32) {
    let count = usize::try_from(count).unwrap_or(usize::MAX);
    if count == 0 || legal.is_empty() {
        work.choices.targets.push(Vec::new());
        return;
    }
    let spent = u64::from(work.choices.random_modes) + u64::from(work.choices.random_targets);
    GameRng::substream(ctx.state.seed, ctx.state.rng_index + spent).shuffle(&mut legal);
    legal.truncate(count);
    work.choices.targets.push(legal);
    work.choices.random_targets = work.choices.random_targets.saturating_add(1);
}

/// Who chooses the targets of `spec`: for "target creature of their choice" the first player it names, in turn order
/// from the controller, who is still in the game; else the controller (CR 601.2c).
fn chooser(ctx: &Ctx<'_>, spec: &TargetSpec) -> PlayerId {
    let Targetable::ChosenBy { who, .. } = &spec.what else {
        return ctx.player;
    };
    let origin = ctx.state.object(ctx.source).map_or(
        ObjectRef {
            id: ctx.source,
            counter: 0,
        },
        GameObject::reference,
    );
    let scope = match ctx.cause() {
        Some(cause) => Scope::triggered(ctx.state, ctx.player, origin, cause),
        None => Scope::announcing(ctx.state, ctx.player, origin),
    };
    let named = scope.players(who).unwrap_or_default();
    ctx.state
        .turn_order_from(ctx.player)
        .into_iter()
        .find(|seat| named.contains(seat))
        .unwrap_or(ctx.player)
}
