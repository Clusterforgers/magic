//! Flagbearers: while an opponent of a permanent's controller with `GameRule::Flagbearers` chooses targets as they cast a
//! spell or activate an ability, they must choose at least one Flagbearer on the battlefield if able. Triggered abilities
//! and copies given new targets are not bound (the Dominaria rulings).
//!
//! The requirement falls on the last target specification that could take a Flagbearer while none is chosen: the earlier
//! ones stay free. There a specification of one target offers only Flagbearers; one of several asks a Flagbearer first
//! and then the rest of its targets.

use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_cards::refs::{TargetSpec, ZoneKind};
use mtg_core::card::Subtype;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Ctx, Done, What};
use crate::ability_reader::abilities_of;
use crate::characteristics::{characteristics, controller_of};
use crate::choose::{Choices, Target, TargetOptions, legal_for};
use crate::state::GameState;
use crate::zones::Zone;

/// The Flagbearer pick of a specification of several targets: asked, then taken.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum R5509Split {
    /// No pick is pending.
    #[default]
    None,
    /// The Flagbearer of the specification is asked.
    Asking,
    /// The Flagbearer was chosen; the rest of the specification's targets are asked next.
    Picked(Target),
}

/// Whether `id` is a Flagbearer on the battlefield: its card prints the creature type or an effect gave it.
pub(crate) fn r5509_is_flagbearer(state: &GameState, id: ObjectId) -> bool {
    let Some(object) = state.object(id) else {
        return false;
    };
    object.zone() == Zone::Battlefield
        && characteristics(state, id).is_some_and(|chars| {
            chars.has_subtype(&object.card().subtypes, &Subtype("Flagbearer".to_owned()))
        })
}

/// Whether a permanent whose controller is an opponent of `chooser` has the Flagbearer rule.
pub(crate) fn r5509_bound(state: &GameState, chooser: PlayerId) -> bool {
    state.zone(Zone::Battlefield).iter().any(|id| {
        controller_of(state, *id).is_some_and(|owner| state.is_opponent(owner, chooser))
            && abilities_of(state, *id).iter().any(|(_, ability)| {
                matches!(
                    &**ability,
                    Ability::Rule {
                        rule: GameRule::Flagbearers,
                        condition: None,
                        from: ZoneKind::Battlefield,
                    }
                )
            })
    })
}

fn is_flag(state: &GameState, target: &Target) -> bool {
    matches!(target, Target::Object(object) if r5509_is_flagbearer(state, object.id))
}

/// Narrows the options of specification `index` when the requirement falls on it: only Flagbearers for a specification
/// of one target, or only them for the first pick of one of several (`Done::r5509_split` turns `Asking`). A pending pick
/// leaves the rest of the specification's targets to choose. Nothing changes when the chooser is not bound, a Flagbearer
/// is already chosen, none can be chosen here or a later specification could still take one.
pub(super) fn r5509_narrow(
    ctx: &Ctx<'_>,
    choices: &Choices,
    done: &mut Done,
    specs: &[TargetSpec],
    index: usize,
    options: &mut TargetOptions,
) {
    if let R5509Split::Picked(pick) = done.r5509_split {
        options.legal.retain(|target| *target != pick);
        options.min = options.min.saturating_sub(1);
        options.max = options.max.saturating_sub(1);
        return;
    }
    let state = ctx.state;
    if ctx.triggered.is_some()
        || !matches!(ctx.what, What::Spell(_) | What::Ability(_))
        || !r5509_bound(state, ctx.player)
        || choices
            .targets
            .iter()
            .flatten()
            .any(|target| is_flag(state, target))
    {
        return;
    }
    let flags: Vec<Target> = options
        .legal
        .iter()
        .filter(|target| is_flag(state, target))
        .copied()
        .collect();
    if flags.is_empty() || options.max == 0 {
        return;
    }
    let later = specs.iter().skip(index + 1).any(|spec| {
        legal_for(
            state,
            spec,
            ctx.source,
            ctx.player,
            ctx.cause(),
            choices.x,
            &choices.targets,
        )
        .iter()
        .any(|target| is_flag(state, target))
    });
    if later {
        return;
    }
    let several = options.max > 1 && flags.len() < options.legal.len();
    options.legal = flags;
    options.min = options.min.max(1);
    if several {
        options.max = 1;
        done.r5509_split = R5509Split::Asking;
    } else {
        options.max = options
            .max
            .min(u32::try_from(options.legal.len()).unwrap_or(u32::MAX));
    }
}

/// Takes the answer for a target specification: the Flagbearer pick is held while the rest are asked, and the rest join
/// it. Returns the targets of the specification once they are complete.
pub(super) fn r5509_take(done: &mut Done, targets: Vec<Target>) -> Option<Vec<Target>> {
    match std::mem::take(&mut done.r5509_split) {
        R5509Split::Asking => {
            done.r5509_split = targets
                .first()
                .map_or(R5509Split::None, |pick| R5509Split::Picked(*pick));
            if targets.is_empty() {
                return Some(targets);
            }
            None
        }
        R5509Split::Picked(pick) => {
            let mut all = vec![pick];
            all.extend(targets);
            Some(all)
        }
        R5509Split::None => Some(targets),
    }
}

/// Completes a pending pick when the rest of the specification has nothing to ask: the pick alone is its targets.
pub(super) fn r5509_finish(done: &mut Done) -> Option<Vec<Target>> {
    match std::mem::take(&mut done.r5509_split) {
        R5509Split::Picked(pick) => Some(vec![pick]),
        other => {
            done.r5509_split = other;
            None
        }
    }
}
