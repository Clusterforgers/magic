//! Multikicker (CR 702.33c): "You may pay an additional [cost] any number of times as you cast this spell."
//!
//! Each payment counts: `Amount::Kicks` is the number of times the cost was paid.

use mtg_cards::keyword::Keyword;

use super::{AdditionalCost, KeywordRules};

/// The rules of multikicker.
#[derive(Debug)]
pub(super) struct Multikicker;

impl KeywordRules for Multikicker {
    fn additional_cost(&self, keyword: &Keyword) -> Option<AdditionalCost> {
        let Keyword::Multikicker(cost) = keyword else {
            return None;
        };
        Some(AdditionalCost {
            name: "Multikicker",
            cost: cost.clone(),
            repeatable: true,
        })
    }
}
