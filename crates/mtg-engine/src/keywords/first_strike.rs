//! First strike (CR 702.7): the creature deals combat damage in the first-strike step, before creatures without it.

use mtg_cards::keyword::Keyword;

use super::{DamageRules, KeywordRules};

/// The rules of first strike.
#[derive(Debug)]
pub(super) struct FirstStrike;

impl KeywordRules for FirstStrike {
    fn modifies_damage(&self, _keyword: &Keyword) -> DamageRules {
        DamageRules {
            first_strike: true,
            ..DamageRules::default()
        }
    }
}
