//! Menace (CR 702.111): a creature with menace can't be blocked except by two or more creatures.

use mtg_cards::keyword::Keyword;

use super::KeywordRules;

/// The rules of menace.
#[derive(Debug)]
pub(super) struct Menace;

impl KeywordRules for Menace {
    fn min_blockers(&self, _keyword: &Keyword) -> usize {
        2
    }
}
