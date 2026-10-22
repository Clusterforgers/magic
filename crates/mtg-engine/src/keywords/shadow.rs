//! Shadow (CR 702.28): the creature can block or be blocked by only creatures with shadow.

use mtg_cards::keyword::Keyword;
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
        !has_keyword(state, blocker, keyword)
    }

    fn restricts_blocked(
        &self,
        state: &GameState,
        keyword: &Keyword,
        attacker: ObjectId,
        _blocker: ObjectId,
    ) -> bool {
        !has_keyword(state, attacker, keyword)
    }
}
