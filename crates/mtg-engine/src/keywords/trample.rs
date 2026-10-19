//! Trample (CR 702.19): an attacker assigns the damage beyond lethal to its blockers to the player or planeswalker it attacks.

use mtg_cards::keyword::Keyword;

use super::{DamageRules, KeywordRules};

/// The rules of trample.
#[derive(Debug)]
pub(super) struct Trample;

impl KeywordRules for Trample {
    fn modifies_damage(&self, _keyword: &Keyword) -> DamageRules {
        DamageRules {
            trample: true,
            ..DamageRules::default()
        }
    }
}
