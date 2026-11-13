//! The replacement effects of a keyword action (explore, connive, proliferate) that an action about to be performed
//! meets: gathered like those of events, matched against the creature or player that would act.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::r406_actions::r406_is_action;
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_core::ids::PlayerId;

use super::gather;
use crate::object::ObjectRef;
use crate::resolve::{ResolveError, Scope};
use crate::state::GameState;

/// Who or what would perform a keyword action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum R406Subject {
    /// A creature would explore.
    Explores(ObjectRef),
    /// A creature would connive.
    Connives(ObjectRef),
    /// A player would proliferate.
    Proliferates(PlayerId),
}

/// A replacement of a keyword action that works now: whose it is and the steps it runs instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct R406Candidate {
    /// The object with the ability.
    pub(crate) object: ObjectRef,
    /// Its controller, whom "you" names in the steps.
    pub(crate) controller: PlayerId,
    /// The event it replaces.
    pub(crate) when: ReplaceWhen,
    /// What happens instead.
    pub(crate) steps: Vec<Effect>,
}

/// The replacements of a keyword action that apply to `subject` now, in the order the game finds them.
pub(crate) fn r406_candidates(
    state: &GameState,
    subject: R406Subject,
) -> Result<Vec<R406Candidate>, ResolveError> {
    let mut found = Vec::new();
    for source in gather(state) {
        let Ability::Replacement {
            when,
            by: ReplaceBy::Instead(steps),
            condition,
            ..
        } = &*source.ability
        else {
            continue;
        };
        if !r406_is_action(when) {
            continue;
        }
        let scope = Scope::announcing(state, source.controller, source.object);
        let fits = match (when, subject) {
            (ReplaceWhen::Explores { what }, R406Subject::Explores(creature))
            | (ReplaceWhen::Connives { what }, R406Subject::Connives(creature)) => {
                scope.selects(what, creature)?
            }
            (ReplaceWhen::Proliferates { who }, R406Subject::Proliferates(player)) => {
                scope.players(who)?.contains(&player)
            }
            _ => false,
        };
        if !fits {
            continue;
        }
        if let Some(condition) = condition
            && !scope.holds(condition)?
        {
            continue;
        }
        found.push(R406Candidate {
            object: source.object,
            controller: source.controller,
            when: when.clone(),
            steps: steps.clone(),
        });
    }
    Ok(found)
}
