//! New targets for a copy of a spell or ability (CR 707.10c): the question and its answers, shared by `CopySpell` and
//! `CopyAbility`.

use std::slice::Iter;

use mtg_cards::ability::Ability;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::abilities::Cause;
use crate::choose::{
    Choice, Choices, Target, TargetOptions, YesNoOptions, announced_specs, legal_from,
};
use crate::decision::{Decision, DecisionKind};
use crate::object::ObjectRef;
use crate::resolve::{Cx, ResolveError};
use crate::wording;

/// What the original spell or ability is, for the questions and the legal targets of its copy.
#[derive(Clone, Copy)]
pub(super) struct Original<'a> {
    /// The spell or ability object.
    pub(super) id: ObjectId,
    /// Its name, for the question.
    pub(super) name: &'a str,
    /// What it announced.
    pub(super) choices: &'a Choices,
    /// Its rules, which say what its targets are.
    pub(super) ability: &'a Ability,
    /// The object its targets are judged from: the spell, or the source of the ability.
    pub(super) origin: ObjectRef,
    /// What triggered it, for a triggered ability.
    pub(super) cause: Option<&'a Cause>,
}

/// How far the choice of new targets has come.
pub(super) enum Retargeted {
    /// The next question.
    Ask(Decision),
    /// What the copy announces: the targets of the original, or the new ones.
    Done(Choices),
}

/// The choices of a copy of `original` made by `player`, who controls the copy.
///
/// Nobody is asked when no target of the original could be another one. Otherwise the controller of the copy is asked
/// whether to choose new targets (No keeps every target of the original, legal or not, CR 707.10c), then
/// `ChooseTargets` for each specification that has another choice, for as many targets as the original had. The answers
/// are read in order from `answers`, the rest of the journal of the step.
pub(super) fn retarget(
    cx: &Cx<'_>,
    player: PlayerId,
    answers: &mut Iter<'_, (PlayerId, Choice)>,
    original: Original<'_>,
    may_choose: bool,
) -> Result<Retargeted, ResolveError> {
    let mut copy = original.choices.clone();
    copy.forget_cost_picks();
    let state = cx.state();
    let specs = announced_specs(original.ability, &original.choices.modes);
    let mut changeable = Vec::new();
    for (at, spec) in specs.iter().enumerate() {
        let Some(kept) = original.choices.targets.get(at).filter(|t| !t.is_empty()) else {
            continue;
        };
        if !may_choose {
            break;
        }
        let legal = legal_from(
            state,
            spec,
            original.origin,
            &[],
            player,
            original.cause,
            original.choices.x,
        );
        let another = legal.len() > kept.len()
            || (legal.len() == kept.len() && !kept.iter().all(|target| legal.contains(target)));
        if another {
            changeable.push((at, wording::spec(spec), legal));
        }
    }
    if changeable.is_empty() {
        return Ok(Retargeted::Done(copy));
    }
    match answers.next() {
        None => {
            return Ok(Retargeted::Ask(Decision {
                player,
                kind: DecisionKind::YesNo(YesNoOptions::new(
                    original.id,
                    format!("Choose new targets for the copy of {}?", original.name),
                )),
            }));
        }
        Some((who, Choice::YesNo(false))) if *who == player => {
            return Ok(Retargeted::Done(copy));
        }
        Some((who, Choice::YesNo(true))) if *who == player => {}
        Some(other) => return Err(unexpected(other)),
    }
    for (at, wording, legal) in changeable {
        let count = original.choices.targets[at].len();
        let wanted = u32::try_from(count).unwrap_or(u32::MAX);
        match answers.next() {
            None => {
                return Ok(Retargeted::Ask(Decision {
                    player,
                    kind: DecisionKind::ChooseTargets(TargetOptions {
                        source: original.id,
                        spec: at,
                        legal,
                        min: wanted,
                        max: wanted,
                        can_cancel: false,
                        wording,
                        triggered: false,
                    }),
                }));
            }
            Some((who, Choice::Targets(targets)))
                if *who == player && fits(targets, &legal, count) =>
            {
                copy.targets[at].clone_from(targets);
            }
            Some(other) => return Err(unexpected(other)),
        }
    }
    copy.division = divided_again(&original.choices.division, &original.choices.targets, &copy);
    Ok(Retargeted::Done(copy))
}

/// Whether `targets` are `count` distinct targets among the legal ones.
fn fits(targets: &[Target], legal: &[Target], count: usize) -> bool {
    targets.len() == count
        && targets
            .iter()
            .enumerate()
            .all(|(at, target)| legal.contains(target) && !targets[..at].contains(target))
}

/// The division of the original shared among the targets of the copy: each new target takes the share of the target it
/// replaces, which is the one in its place among all the targets.
fn divided_again(
    division: &[(Target, u32)],
    old: &[Vec<Target>],
    copy: &Choices,
) -> Vec<(Target, u32)> {
    let before: Vec<Target> = old.iter().flatten().copied().collect();
    let after: Vec<Target> = copy.targets.iter().flatten().copied().collect();
    division
        .iter()
        .map(|(target, share)| {
            let at = before.iter().position(|named| named == target);
            let renamed = at.and_then(|at| after.get(at)).copied();
            (renamed.unwrap_or(*target), *share)
        })
        .collect()
}

fn unexpected(answer: &(PlayerId, Choice)) -> ResolveError {
    ResolveError::Invalid(format!(
        "{answer:?} is no answer to the new targets of a copy"
    ))
}
