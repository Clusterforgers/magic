//! The replacements of a scry or a surveil that a player about to scry or surveil meets: gathered like those of
//! events, matched against the player.

use mtg_cards::ability::Ability;
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_core::ids::PlayerId;

use super::gather;
use crate::object::ObjectRef;
use crate::resolve::{ResolveError, Scope};
use crate::state::GameState;

/// A replacement of a scry or a surveil that works now for a player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct R4708Candidate {
    /// The object with the ability.
    pub(crate) object: ObjectRef,
    /// Its controller.
    pub(crate) controller: PlayerId,
    /// How many more cards it offers (a surveil); zero for a scry replaced by draws.
    pub(crate) more: usize,
}

/// The replacements of a scry (`surveil` false) or a surveil (`surveil` true) by `player` that apply now, in the order
/// the game finds them.
pub(crate) fn r4708_candidates(
    state: &GameState,
    player: PlayerId,
    surveil: bool,
) -> Result<Vec<R4708Candidate>, ResolveError> {
    let mut found = Vec::new();
    for source in gather(state) {
        let Ability::Replacement {
            when,
            by,
            condition,
            ..
        } = &*source.ability
        else {
            continue;
        };
        let scope = Scope::announcing(state, source.controller, source.object);
        let more = match (when, by, surveil) {
            (ReplaceWhen::Scries { who }, ReplaceBy::Instead(_), false) => {
                if !scope.players(who)?.contains(&player) {
                    continue;
                }
                0
            }
            (ReplaceWhen::Surveils { who }, ReplaceBy::MayAddAmount(amount), true) => {
                if !scope.players(who)?.contains(&player) {
                    continue;
                }
                usize::try_from(scope.amount(amount)?.max(0)).unwrap_or(0)
            }
            _ => continue,
        };
        if let Some(condition) = condition
            && !scope.holds(condition)?
        {
            continue;
        }
        found.push(R4708Candidate {
            object: source.object,
            controller: source.controller,
            more,
        });
    }
    Ok(found)
}
