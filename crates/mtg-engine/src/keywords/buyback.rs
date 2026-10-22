//! Buyback (CR 702.27): "You may pay an additional [cost] as you cast this spell. If you do, put this spell into its
//! owner's hand instead of into its owner's graveyard as it resolves."
//!
//! The cost is announced with the cast (`announce/optional.rs`); the replacement step returns the spell that resolved
//! to its owner's hand (`replace.rs`), and a spell that was countered or did not resolve goes to the graveyard.

use mtg_cards::keyword::Keyword;

use super::{AdditionalCost, KeywordRules, OptionalKind};

/// The rules of buyback.
#[derive(Debug)]
pub(super) struct Buyback;

impl KeywordRules for Buyback {
    fn additional_cost(&self, keyword: &Keyword) -> Option<AdditionalCost> {
        let Keyword::Buyback(cost) = keyword else {
            return None;
        };
        Some(AdditionalCost {
            name: "Buyback",
            cost: cost.clone(),
            repeatable: false,
            kind: OptionalKind::Buyback,
        })
    }
}
