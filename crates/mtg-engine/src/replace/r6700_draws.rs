//! "If you would draw one or more cards, you draw that many cards plus one instead" (`ReplaceWhen::R6700Draws`, Quantum
//! Riddler): a replacement of a whole draw instruction, applied before any replacement of a card drawn (its ruling), so
//! the draw steps and the draw effects ask it for the count before they make the draws.

use mtg_cards::ability::Ability;
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_core::ids::PlayerId;

use super::gather;
use crate::resolve::Scope;
use crate::state::GameState;

/// How many cards `player` draws for an instruction to draw `count`: each replacement of a draw instruction that
/// applies to them adds its number once (CR 616.5: each applies once to the event); a draw of none is no draw.
pub(crate) fn r6700_draw_count(state: &GameState, player: PlayerId, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    let extra: usize = gather(state)
        .iter()
        .filter_map(|source| {
            let Ability::Replacement {
                when: ReplaceWhen::R6700Draws { who },
                by: ReplaceBy::Add(more),
                condition,
                ..
            } = &*source.ability
            else {
                return None;
            };
            let scope = Scope::announcing(state, source.controller, source.object);
            let applies = scope
                .players(who)
                .is_ok_and(|players| players.contains(&player))
                && condition
                    .as_ref()
                    .is_none_or(|condition| scope.holds(condition).unwrap_or(false));
            applies.then(|| usize::try_from(*more).unwrap_or(0))
        })
        .sum();
    count.saturating_add(extra)
}
