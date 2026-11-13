//! The replacements of a copy of a spell ("If you would copy a spell one or more times, instead copy it that many times
//! plus an additional time."): gathered like those of events, matched against the player who copies.

use mtg_cards::ability::Ability;
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_core::ids::PlayerId;

use super::gather;
use crate::resolve::{ResolveError, Scope};
use crate::state::GameState;

/// How many copies the replacements that apply now add when `player` would copy a spell one or more times: each one
/// that applies adds its number once (CR 616.1, each modifies the event the others left).
pub(crate) fn r5516_added_copies(state: &GameState, player: PlayerId) -> Result<u32, ResolveError> {
    let mut added: u32 = 0;
    for source in gather(state) {
        let Ability::Replacement {
            when: ReplaceWhen::CopiesSpell { who },
            by: ReplaceBy::AddAmount(amount),
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
        let more = u32::try_from(scope.amount(amount)?.max(0)).unwrap_or(0);
        added = added.saturating_add(more);
    }
    Ok(added)
}
