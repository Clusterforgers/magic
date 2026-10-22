//! Toxic N (CR 702.164): players dealt combat damage by the creature also get N poison counters; several instances add up.

use mtg_cards::keyword::Keyword;

use super::{DamageRules, KeywordRules};

/// The rules of toxic.
#[derive(Debug)]
pub(super) struct Toxic;

impl KeywordRules for Toxic {
    fn modifies_damage(&self, keyword: &Keyword) -> DamageRules {
        let Keyword::Toxic(poison) = keyword else {
            return DamageRules::default();
        };
        DamageRules {
            toxic: *poison,
            ..DamageRules::default()
        }
    }
}
