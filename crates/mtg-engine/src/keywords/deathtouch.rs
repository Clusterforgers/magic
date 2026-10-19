//! Deathtouch (CR 702.2): any damage the creature deals to a creature is lethal, and destroys it at the next state-based check.

use mtg_cards::keyword::Keyword;

use super::{DamageRules, KeywordRules};

/// The rules of deathtouch.
#[derive(Debug)]
pub(super) struct Deathtouch;

impl KeywordRules for Deathtouch {
    fn modifies_damage(&self, _keyword: &Keyword) -> DamageRules {
        DamageRules {
            deathtouch: true,
            ..DamageRules::default()
        }
    }
}
