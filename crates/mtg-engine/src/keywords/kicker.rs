//! Kicker (CR 702.33): "You may pay an additional [cost] as you cast this spell."
//!
//! The cost is announced with the cast (`announce/optional.rs`); the spell and the permanent it becomes remember
//! that it was paid for, and `Condition::Kicked` and `Amount::Kicks` read it.

use mtg_cards::keyword::Keyword;

use super::{AdditionalCost, KeywordRules, OptionalKind};

/// The rules of kicker.
#[derive(Debug)]
pub(super) struct Kicker;

impl KeywordRules for Kicker {
    fn additional_cost(&self, keyword: &Keyword) -> Option<AdditionalCost> {
        let Keyword::Kicker(cost) = keyword else {
            return None;
        };
        Some(AdditionalCost {
            name: "Kicker",
            cost: cost.clone(),
            repeatable: false,
            kind: OptionalKind::Kicker,
        })
    }
}
