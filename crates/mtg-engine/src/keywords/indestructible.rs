//! Indestructible (CR 702.12): destruction by lethal damage, deathtouch or "destroy" does not happen; zero toughness still kills.

use mtg_cards::keyword::Keyword;

use super::KeywordRules;

/// The rules of indestructible.
#[derive(Debug)]
pub(super) struct Indestructible;

impl KeywordRules for Indestructible {
    fn is_indestructible(&self, _keyword: &Keyword) -> bool {
        true
    }
}
