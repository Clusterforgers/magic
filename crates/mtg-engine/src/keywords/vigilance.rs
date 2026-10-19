//! Vigilance (CR 702.20): attacking doesn't cause a creature with vigilance to tap.

use mtg_cards::keyword::Keyword;

use super::KeywordRules;

/// The rules of vigilance.
#[derive(Debug)]
pub(super) struct Vigilance;

impl KeywordRules for Vigilance {
    fn attacks_untapped(&self, _keyword: &Keyword) -> bool {
        true
    }
}
