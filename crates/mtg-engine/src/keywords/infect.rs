//! Infect (CR 702.90): the creature deals damage to creatures in the form of -1/-1 counters and to players in the form of
//! poison counters.

use mtg_cards::keyword::Keyword;

use super::{DamageRules, KeywordRules};

/// The rules of infect.
#[derive(Debug)]
pub(super) struct Infect;

impl KeywordRules for Infect {
    fn modifies_damage(&self, _keyword: &Keyword) -> DamageRules {
        DamageRules {
            infect: true,
            ..DamageRules::default()
        }
    }
}
