//! Wither (CR 702.80): the source deals damage to creatures in the form of -1/-1 counters.

use mtg_cards::keyword::Keyword;

use super::{DamageRules, KeywordRules};

/// The rules of wither.
#[derive(Debug)]
pub(super) struct Wither;

impl KeywordRules for Wither {
    fn modifies_damage(&self, _keyword: &Keyword) -> DamageRules {
        DamageRules {
            wither: true,
            ..DamageRules::default()
        }
    }
}
