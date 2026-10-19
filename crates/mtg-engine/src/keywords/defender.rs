//! Defender (CR 702.3): a creature with defender can't attack.

use mtg_cards::keyword::Keyword;

use super::KeywordRules;

/// The rules of defender.
#[derive(Debug)]
pub(super) struct Defender;

impl KeywordRules for Defender {
    fn forbids_attacking(&self, _keyword: &Keyword) -> bool {
        true
    }
}
