//! The replacements of a learn that a player about to learn meets ("As long as this card is in your graveyard, if you
//! would learn, you may instead return this card to the battlefield."): gathered like those of events.

use mtg_cards::ability::Ability;
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_core::ids::PlayerId;

use super::gather;
use crate::object::ObjectRef;
use crate::resolve::{ResolveError, Scope};
use crate::state::GameState;

/// The objects whose replacement may stand in for a learn of `player` now, each with its controller, in the order the
/// game finds them.
pub(crate) fn r5516_learn_candidates(
    state: &GameState,
    player: PlayerId,
) -> Result<Vec<(ObjectRef, PlayerId)>, ResolveError> {
    let mut found = Vec::new();
    for source in gather(state) {
        let Ability::Replacement {
            when: ReplaceWhen::Learns { who },
            by: ReplaceBy::MayInstead(_),
            condition,
            ..
        } = &*source.ability
        else {
            continue;
        };
        let scope = Scope::announcing(state, source.controller, source.object);
        if !scope.players(who)?.contains(&player) {
            continue;
        }
        if let Some(condition) = condition
            && !scope.holds(condition)?
        {
            continue;
        }
        found.push((source.object, source.controller));
    }
    Ok(found)
}
