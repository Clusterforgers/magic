//! Horsemanship (CR 702.31): the creature can't be blocked except by creatures with horsemanship.

use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;

use super::{KeywordRules, has_keyword};
use crate::state::GameState;

/// The rules of horsemanship.
#[derive(Debug)]
pub(super) struct Horsemanship;

impl KeywordRules for Horsemanship {
    fn restricts_blocking(
        &self,
        state: &GameState,
        keyword: &Keyword,
        _attacker: ObjectId,
        blocker: ObjectId,
    ) -> bool {
        !has_keyword(state, blocker, keyword)
    }
}
