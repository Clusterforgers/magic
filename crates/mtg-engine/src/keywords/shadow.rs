//! Shadow (CR 702.28): the creature can block or be blocked by only creatures with shadow.

use mtg_cards::keyword::Keyword;
use mtg_cards::modify::CombatQuirk;
use mtg_core::ids::ObjectId;

use super::{KeywordRules, has_keyword};
use crate::state::GameState;

/// The rules of shadow.
#[derive(Debug)]
pub(super) struct Shadow;

impl KeywordRules for Shadow {
    fn restricts_blocking(
        &self,
        state: &GameState,
        keyword: &Keyword,
        _attacker: ObjectId,
        blocker: ObjectId,
    ) -> bool {
        !has_keyword(state, blocker, keyword) && !blocks_shadow(state, blocker)
    }

    fn restricts_blocked(
        &self,
        state: &GameState,
        keyword: &Keyword,
        attacker: ObjectId,
        blocker: ObjectId,
    ) -> bool {
        // An attacker blocked as though it had no shadow is one a creature with shadow can't block.
        !has_keyword(state, attacker, keyword) || ignores_shadow(state, blocker)
    }
}

/// Whether a rule lets the blocker block creatures with shadow, as though it had shadow or they had none.
fn blocks_shadow(state: &GameState, blocker: ObjectId) -> bool {
    state.rules().quirks.has(blocker, |quirk| {
        matches!(
            quirk,
            CombatQuirk::BlockAsThoughShadow | CombatQuirk::BlockIgnoringShadow
        )
    })
}

/// Whether the blocker blocks creatures with shadow as though they had none.
fn ignores_shadow(state: &GameState, blocker: ObjectId) -> bool {
    state.rules().quirks.has(blocker, |quirk| {
        matches!(quirk, CombatQuirk::BlockIgnoringShadow)
    })
}
