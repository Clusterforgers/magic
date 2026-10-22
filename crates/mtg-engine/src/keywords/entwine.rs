//! Entwine (CR 702.42): "You may choose all modes of this spell instead of just one. If you do, you pay the entwine
//! cost in addition to its other costs."
//!
//! The cost is announced with the cast, before the modes (`announce/optional.rs`); when it is paid every mode is chosen
//! and none is asked (`announce/modes.rs`).

use mtg_cards::keyword::Keyword;

use super::{AdditionalCost, KeywordRules, OptionalKind};

/// The rules of entwine.
#[derive(Debug)]
pub(super) struct Entwine;

impl KeywordRules for Entwine {
    fn additional_cost(&self, keyword: &Keyword) -> Option<AdditionalCost> {
        let Keyword::Entwine(cost) = keyword else {
            return None;
        };
        Some(AdditionalCost {
            name: "Entwine",
            cost: cost.clone(),
            repeatable: false,
            kind: OptionalKind::Entwine,
        })
    }
}
