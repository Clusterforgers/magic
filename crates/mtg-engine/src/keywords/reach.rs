//! Reach (CR 702.17): a creature with reach can block creatures with flying.

use mtg_cards::keyword::Keyword;

use super::KeywordRules;

/// The rules of reach.
#[derive(Debug)]
pub(super) struct Reach;

impl KeywordRules for Reach {
    fn answers_evasion(&self, _keyword: &Keyword, evasion: &Keyword) -> bool {
        *evasion == Keyword::Flying
    }
}
