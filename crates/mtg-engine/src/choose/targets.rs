//! `ChooseTargets`: who and what a target specification may name, the decision that picks them, and the
//! recheck on resolution that uses the same rule (CR 115, 608.2b).

use mtg_cards::refs::{TargetSpec, Targetable};
use mtg_cards::value::Amount;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use super::Target;
use crate::answers::Outcome;
use crate::error::{EngineError, IllegalAnswer};
use crate::keywords;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{ResolveError, Scope};
use crate::state::GameState;
use crate::zones::Zone;

/// The targets a player may choose for one target specification of a spell or ability being put on the stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TargetOptions {
    /// The spell or ability.
    pub source: ObjectId,
    /// Which of its target specifications this is, from 0.
    pub spec: usize,
    /// Everything that may be chosen.
    pub legal: Vec<Target>,
    /// The fewest targets to choose.
    pub min: u32,
    /// The most targets to choose, never more than there are to choose from.
    pub max: u32,
    /// Whether the cast or activation may still be backed out of: nothing is logged until it is paid for.
    pub can_cancel: bool,
}

/// The answer to a target decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum TargetChoice {
    /// These targets, each once, between `min` and `max` of them, all among the legal ones.
    Targets(Vec<Target>),
    /// Back out of the cast or activation (when `can_cancel`).
    Cancel,
}

fn candidates(scope: &Scope<'_>, what: &Targetable) -> Result<Vec<Target>, ResolveError> {
    let state = scope.state;
    let players = || state.alive_players().map(Target::Player);
    Ok(match what {
        Targetable::AnyTarget => players()
            .chain(
                state
                    .zone(Zone::Battlefield)
                    .iter()
                    .filter_map(|id| state.object(*id))
                    .filter(|o| scope.can_be_dealt_damage(o.reference()))
                    .map(|o| Target::Object(o.reference())),
            )
            .collect(),
        Targetable::Player => players().collect(),
        Targetable::Opponent => players()
            .filter(|t| t.player() != Some(scope.controller))
            .collect(),
        Targetable::Object(selector) => scope
            .matching(selector)?
            .into_iter()
            .map(Target::Object)
            .collect(),
        Targetable::Either(kinds) => {
            let mut all = Vec::new();
            for kind in kinds {
                all.extend(candidates(scope, kind)?);
            }
            all
        }
    })
}

/// Everything `spec` lets a spell or ability of `source`, controlled by `controller`, target right now.
///
/// One rule serves both choosing and the recheck on resolution (CR 608.2b): a target is still legal when it
/// is still in this list. An object that changed zones is a new object, so it is not. A selector the engine
/// cannot evaluate matches nothing, so the spell is not offered. Protection (hexproof, ward) joins here.
pub fn legal_targets(
    state: &GameState,
    spec: &TargetSpec,
    source: ObjectId,
    controller: PlayerId,
) -> Vec<Target> {
    let source = state.object(source).map_or(
        ObjectRef {
            id: source,
            counter: 0,
        },
        GameObject::reference,
    );
    let scope = Scope::announcing(state, controller, source);
    let mut legal = candidates(&scope, &spec.what).unwrap_or_default();
    legal.retain(|target| match target {
        Target::Object(object) => !keywords::protects(state, object.id, source.id, controller),
        Target::Player(_) => true,
    });
    legal.sort_unstable();
    legal.dedup();
    legal
}

/// Whether `target` is among the legal targets of `spec`.
pub fn is_legal(
    state: &GameState,
    spec: &TargetSpec,
    target: Target,
    source: ObjectId,
    controller: PlayerId,
) -> bool {
    legal_targets(state, spec, source, controller).contains(&target)
}

/// The decision for the specification `index`, or `None` when it cannot be met: fewer legal targets than its minimum.
pub(crate) fn options_for(
    state: &GameState,
    source: ObjectId,
    controller: PlayerId,
    index: usize,
    spec: &TargetSpec,
    can_cancel: bool,
) -> Option<TargetOptions> {
    let legal = legal_targets(state, spec, source, controller);
    let available = u32::try_from(legal.len()).unwrap_or(u32::MAX);
    if available < spec.bounds.min {
        return None;
    }
    let max = match &spec.bounds.max {
        None => available,
        Some(Amount::Fixed(n)) => u32::try_from(*n).unwrap_or(0),
        Some(other) => {
            let origin = state.object(source).map_or(
                ObjectRef {
                    id: source,
                    counter: 0,
                },
                GameObject::reference,
            );
            let amount = Scope::announcing(state, controller, origin)
                .amount(other)
                .ok()?;
            u32::try_from(amount).unwrap_or(0)
        }
    };
    Some(TargetOptions {
        source,
        spec: index,
        legal,
        min: spec.bounds.min,
        max: max.min(available),
        can_cancel,
    })
}

/// Whether the specification asks for any choice at all: a specification that allows no targets, or has none
/// to offer and needs none, is passed over.
pub(crate) fn asks(options: &TargetOptions) -> bool {
    options.max > 0 && !options.legal.is_empty()
}

/// Checks the answer against the options it answers.
pub(crate) fn answer(
    _player: PlayerId,
    options: &TargetOptions,
    choice: TargetChoice,
) -> Result<Outcome, EngineError> {
    let targets = match choice {
        TargetChoice::Cancel if options.can_cancel => return Ok(Outcome::CancelCast),
        TargetChoice::Cancel => return Err(IllegalAnswer::NotOffered("cancel").into()),
        TargetChoice::Targets(targets) => targets,
    };
    for (at, target) in targets.iter().enumerate() {
        if targets[..at].contains(target) {
            return Err(IllegalAnswer::Duplicate("target").into());
        }
        if !options.legal.contains(target) {
            return Err(IllegalAnswer::NotOffered("target").into());
        }
    }
    let count = u32::try_from(targets.len()).unwrap_or(u32::MAX);
    let wanted = if count < options.min {
        Some(options.min)
    } else if count > options.max {
        Some(options.max)
    } else {
        None
    };
    if let Some(expected) = wanted {
        return Err(IllegalAnswer::WrongCount {
            expected: usize::try_from(expected).unwrap_or(usize::MAX),
            got: targets.len(),
        }
        .into());
    }
    Ok(Outcome::Targets(targets))
}
