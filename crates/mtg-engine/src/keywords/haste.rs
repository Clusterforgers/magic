//! Haste (CR 702.10): a creature with haste can attack and use `{T}` abilities the turn it comes under its controller's control.

use mtg_cards::keyword::Keyword;

use super::KeywordRules;

/// The rules of haste.
#[derive(Debug)]
pub(super) struct Haste;

impl KeywordRules for Haste {
    fn ignores_summoning_sickness(&self, _keyword: &Keyword) -> bool {
        true
    }
}
