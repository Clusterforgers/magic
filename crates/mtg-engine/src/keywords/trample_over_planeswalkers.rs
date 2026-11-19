//! Trample over planeswalkers (CR 702.19c): trample, and attacking a planeswalker the damage beyond lethal to its blockers
//! and to the planeswalker goes to that planeswalker's controller.

use mtg_cards::keyword::Keyword;

use super::{DamageRules, KeywordRules};

/// The rules of trample over planeswalkers.
#[derive(Debug)]
pub(super) struct TrampleOverPlaneswalkers;

impl KeywordRules for TrampleOverPlaneswalkers {
    fn modifies_damage(&self, _keyword: &Keyword) -> DamageRules {
        DamageRules {
            trample: true,
            over_planeswalkers: true,
            ..DamageRules::default()
        }
    }
}
