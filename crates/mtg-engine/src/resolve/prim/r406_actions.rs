//! A keyword action (explore, connive, proliferate) that meets a replacement of it (CR 614.1a, 616): the steps that
//! happen instead, worked out once and run one per pass of the step.
//!
//! The first pass works out the plan: for each creature or player in turn, a replacement not yet applied to the
//! action is applied (its affected player chooses among replacements that differ, CR 616.1), and each `Explore`,
//! `Connive` or `Proliferate` among its steps is the modified action, which the other replacements may still change
//! but not this one (CR 614.5). The plan is bound under a name the step's position keys and holds the steps around the
//! action, each with its controller and the creature or player it is about (`Triggering`), and the bare actions. Each
//! pass after runs the first step left on the state the earlier ones leave (`Next::Again`), so a scry before an
//! exploration changes the card it reveals.

use mtg_cards::effect::Effect;
use mtg_cards::r406_actions::r406_is_replaced;
use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use super::connive::connivances;
use super::explore::{Actor, actor_of, explorations};
use super::proliferate::proliferations;
use crate::abilities::Cause;
use crate::choose::{ChoiceOption, Questions, Stop, Target};
use crate::replace::r406_actions::{R406Candidate, R406Subject, r406_candidates};
use crate::resolve::{CLEAR, Cx, Next, ResolveError, Run};

/// The prefix of the name a plan is bound under.
const R406_PLAN: &str = "~r406 plan ";

/// One step of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum R406Item {
    /// The action itself, which nothing replaces any more: the creature that explores or connives (`count` cards),
    /// or the player who proliferates.
    Act { subject: Target, count: usize },
    /// A step of a replacement, run for its controller about the creature or player that would have acted.
    Around {
        effect: Box<Effect>,
        controller: PlayerId,
        subject: Target,
    },
}

/// The key of the plan of the step at this position.
fn r406_key(cx: &Cx<'_>) -> String {
    let path: Vec<String> = cx
        .resolving
        .position()
        .iter()
        .map(ToString::to_string)
        .collect();
    format!("{R406_PLAN}{}\t", path.join("."))
}

/// The plan bound for this step, with the name it is bound under.
fn r406_bound(cx: &Cx<'_>) -> Result<Option<(String, Vec<R406Item>)>, ResolveError> {
    let key = r406_key(cx);
    let Some(name) = cx
        .resolving
        .bindings
        .keys()
        .find(|name| name.starts_with(&key))
    else {
        return Ok(None);
    };
    let plan = serde_json::from_str(&name[key.len()..])
        .map_err(|_| ResolveError::Invalid(format!("`{name}` names no plan of steps")))?;
    Ok(Some((name.clone(), plan)))
}

/// The run of an exploration or a connive of these creatures when a replacement of it applies or a plan is under
/// way; `None` when the action happens as it is.
pub(super) fn r406_replaced(
    cx: &Cx<'_>,
    effect: &Effect,
    actors: &[Actor],
) -> Result<Option<Run>, ResolveError> {
    if let Some((name, plan)) = r406_bound(cx)? {
        return r406_next(cx, effect, &name, plan).map(Some);
    }
    let count = match effect {
        Effect::Connive { count, .. } => {
            usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(0)
        }
        _ => 1,
    };
    let mut subjects = Vec::new();
    for actor in actors {
        let named = actor.permanent.or_else(|| {
            cx.state()
                .object(actor.object)
                .map(crate::object::GameObject::reference)
        });
        let subject = match (effect, named) {
            (Effect::Explore { .. }, Some(named)) => Some(R406Subject::Explores(named)),
            (Effect::Connive { .. }, Some(named)) => Some(R406Subject::Connives(named)),
            _ => None,
        };
        let target = named.map_or(Target::Player(actor.player), Target::Object);
        subjects.push((subject, target, actor.player));
    }
    r406_plan(cx, &subjects, count)
}

/// The run of a proliferation of these players when a replacement of it applies or a plan is under way.
pub(super) fn r406_proliferated(
    cx: &Cx<'_>,
    players: &[PlayerId],
) -> Result<Option<Run>, ResolveError> {
    if let Some((name, plan)) = r406_bound(cx)? {
        let effect = Effect::Proliferate {
            who: mtg_cards::refs::Ref::You,
        };
        return r406_next(cx, &effect, &name, plan).map(Some);
    }
    let subjects: Vec<_> = players
        .iter()
        .map(|player| {
            (
                Some(R406Subject::Proliferates(*player)),
                Target::Player(*player),
                *player,
            )
        })
        .collect();
    r406_plan(cx, &subjects, 1)
}

/// The first pass: the plan of every subject in turn, bound when any replacement applies, asking nothing else.
fn r406_plan(
    cx: &Cx<'_>,
    subjects: &[(Option<R406Subject>, Target, PlayerId)],
    count: usize,
) -> Result<Option<Run>, ResolveError> {
    let state = cx.state();
    let mut found = Vec::new();
    let mut any = false;
    for (subject, _, _) in subjects {
        let candidates = match subject {
            Some(subject) => r406_candidates(state, *subject)?,
            None => Vec::new(),
        };
        any |= !candidates.is_empty();
        found.push(candidates);
    }
    if !any {
        return Ok(None);
    }
    let mut questions = Questions::new(cx);
    let mut plan = Vec::new();
    let walked: Result<(), Stop> =
        subjects
            .iter()
            .zip(&found)
            .try_for_each(|((_, target, chooser), candidates)| {
                r406_expand(
                    cx,
                    &mut questions,
                    (*target, *chooser, count),
                    candidates,
                    &mut Vec::new(),
                    &mut plan,
                )
            });
    if let Err(stop) = walked {
        return Stop::settle(Err(stop)).map(Some);
    }
    questions.end()?;
    let json = serde_json::to_string(&plan)
        .map_err(|_| ResolveError::Invalid("a plan of steps that cannot be written".into()))?;
    Ok(Some(Run::Done {
        events: Vec::new(),
        bound: vec![(format!("{}{json}", r406_key(cx)), Vec::new())],
        snapshots: Vec::new(),
        next: Next::Again,
    }))
}

/// Applies the replacements of `candidates` not yet in `applied` to one action of `subject`, adding its steps to
/// `plan`: the action as it is when none is left, else the steps of the one chosen with each action among them
/// expanded again.
fn r406_expand(
    cx: &Cx<'_>,
    questions: &mut Questions<'_>,
    (subject, chooser, count): (Target, PlayerId, usize),
    candidates: &[R406Candidate],
    applied: &mut Vec<usize>,
    plan: &mut Vec<R406Item>,
) -> Result<(), Stop> {
    let open: Vec<usize> = (0..candidates.len())
        .filter(|index| !applied.contains(index))
        .collect();
    let Some(&first) = open.first() else {
        plan.push(R406Item::Act { subject, count });
        return Ok(());
    };
    let alike = open.iter().all(|index| {
        candidates[*index].steps == candidates[first].steps
            && candidates[*index].controller == candidates[first].controller
    });
    let chosen = if alike {
        first
    } else {
        let state = cx.state();
        let options = open
            .iter()
            .map(|index| {
                ChoiceOption::Label(
                    state
                        .object(candidates[*index].object.id)
                        .map_or_else(String::new, |object| object.card().name.clone()),
                )
            })
            .collect();
        let at = questions.w9b_option(
            chooser,
            options,
            "Choose the replacement effect to apply first".to_owned(),
        )?;
        open.get(at).copied().unwrap_or(first)
    };
    let candidate = &candidates[chosen];
    applied.push(chosen);
    for step in &candidate.steps {
        if r406_is_replaced(&candidate.when, step) {
            r406_expand(
                cx,
                questions,
                (subject, chooser, count),
                candidates,
                applied,
                plan,
            )?;
        } else {
            plan.push(R406Item::Around {
                effect: Box::new(step.clone()),
                controller: candidate.controller,
                subject,
            });
        }
    }
    applied.retain(|index| *index != chosen);
    Ok(())
}

/// A pass after the first: the first step left in the plan, then the plan without it.
fn r406_next(
    cx: &Cx<'_>,
    effect: &Effect,
    name: &str,
    mut plan: Vec<R406Item>,
) -> Result<Run, ResolveError> {
    if plan.is_empty() {
        return Ok(Run::Done {
            events: Vec::new(),
            bound: vec![(format!("{CLEAR}{name}"), Vec::new())],
            snapshots: Vec::new(),
            next: Next::Sibling,
        });
    }
    let item = plan.remove(0);
    // A step run on a later pass binds no "if you do" of its own: the position is the action's, which binds it once.
    let (events, mut bound) = match r406_item(cx, effect, &item)? {
        Run::Done { events, bound, .. } => (
            events,
            bound
                .into_iter()
                .filter(|(name, _)| !name.starts_with("?performed"))
                .collect::<Vec<_>>(),
        ),
        other => return Ok(other),
    };
    bound.push((format!("{CLEAR}{name}"), Vec::new()));
    let next = if plan.is_empty() {
        Next::Sibling
    } else {
        let json = serde_json::to_string(&plan)
            .map_err(|_| ResolveError::Invalid("a plan of steps that cannot be written".into()))?;
        bound.push((format!("{}{json}", r406_key(cx)), Vec::new()));
        Next::Again
    };
    Ok(Run::Done {
        events,
        bound,
        snapshots: Vec::new(),
        next,
    })
}

/// Runs one step of a plan.
fn r406_item(cx: &Cx<'_>, effect: &Effect, item: &R406Item) -> Result<Run, ResolveError> {
    match item {
        R406Item::Act { subject, count } => match (effect, subject) {
            (Effect::Proliferate { .. }, Target::Player(player)) => {
                Stop::settle(proliferations(cx, &[*player]).map(Run::events))
            }
            (Effect::Explore { .. }, Target::Object(named)) => {
                let actors: Vec<Actor> = actor_of(cx, *named).into_iter().collect();
                Stop::settle(explorations(cx, &actors).map(Run::events))
            }
            (Effect::Connive { .. }, Target::Object(named)) => {
                let actors: Vec<Actor> = actor_of(cx, *named).into_iter().collect();
                Stop::settle(connivances(cx, &actors, *count).map(Run::events))
            }
            _ => Ok(Run::events(Vec::new())),
        },
        R406Item::Around {
            effect: step,
            controller,
            subject,
        } => {
            let cause = Cause {
                subjects: vec![*subject],
                ..Cause::default()
            };
            let mut around = cx.clone();
            around.scope.controller = *controller;
            around.scope.cause = Some(&cause);
            super::dispatch(&around, step)
        }
    }
}
