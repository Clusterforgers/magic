//! Flash (CR 702.36): a spell with flash is cast whenever its controller has priority, as an instant is.

use mtg_cards::keyword::Keyword;

use super::KeywordRules;

/// The rules of flash.
#[derive(Debug)]
pub(super) struct Flash;

impl KeywordRules for Flash {
    fn grants_timing(&self, _keyword: &Keyword) -> bool {
        true
    }
}
