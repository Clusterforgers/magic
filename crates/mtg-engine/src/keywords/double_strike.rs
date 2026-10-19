//! Double strike (CR 702.4): the creature deals combat damage in the first-strike step and again in the regular one.

use mtg_cards::keyword::Keyword;

use super::{DamageRules, KeywordRules};

/// The rules of double strike.
#[derive(Debug)]
pub(super) struct DoubleStrike;

impl KeywordRules for DoubleStrike {
    fn modifies_damage(&self, _keyword: &Keyword) -> DamageRules {
        DamageRules {
            double_strike: true,
            ..DamageRules::default()
        }
    }
}
