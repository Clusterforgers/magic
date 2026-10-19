//! Lifelink (CR 702.15): damage the creature deals also gains its controller that much life.

use mtg_cards::keyword::Keyword;

use super::{DamageRules, KeywordRules};

/// The rules of lifelink.
#[derive(Debug)]
pub(super) struct Lifelink;

impl KeywordRules for Lifelink {
    fn modifies_damage(&self, _keyword: &Keyword) -> DamageRules {
        DamageRules {
            lifelink: true,
            ..DamageRules::default()
        }
    }
}
