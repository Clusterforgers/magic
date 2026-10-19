//! Flying (CR 702.9): a creature with flying can't be blocked except by creatures with flying or reach.

use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;

use super::{KeywordRules, keywords_of, rules};
use crate::state::GameState;

/// The rules of flying.
#[derive(Debug)]
pub(super) struct Flying;

impl KeywordRules for Flying {
    fn restricts_blocking(
        &self,
        state: &GameState,
        keyword: &Keyword,
        _attacker: ObjectId,
        blocker: ObjectId,
    ) -> bool {
        !keywords_of(state, blocker)
            .into_iter()
            .any(|held| rules(held).answers_evasion(held, keyword))
    }

    fn answers_evasion(&self, _keyword: &Keyword, evasion: &Keyword) -> bool {
        *evasion == Keyword::Flying
    }
}
