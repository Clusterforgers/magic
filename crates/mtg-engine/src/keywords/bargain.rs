//! Bargain (CR 702.166): "As an additional cost to cast this spell, you may sacrifice an artifact, enchantment, or
//! token."
//!
//! The cost is an optional additional cost read as `Condition::AdditionalCostPaid` ("if this spell was bargained").

use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;

use super::{AdditionalCost, KeywordRules, OptionalKind};

/// The rules of bargain.
#[derive(Debug)]
pub(super) struct Bargain;

impl KeywordRules for Bargain {
    fn additional_cost(&self, keyword: &Keyword) -> Option<AdditionalCost> {
        if !matches!(keyword, Keyword::Bargain) {
            return None;
        }
        Some(AdditionalCost {
            name: "Bargain",
            cost: Cost(vec![CostItem::Sacrifice {
                filter: Selector(vec![Pred::AnyOf(vec![
                    Pred::Type(CardType::Artifact),
                    Pred::Type(CardType::Enchantment),
                    Pred::Token,
                ])]),
                count: Amount::one(),
            }]),
            repeatable: false,
            kind: OptionalKind::Additional,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Bargain offers one optional cost that sacrifices an artifact, an enchantment or a token and nothing else, and no other keyword does.
    use rstest::rstest;

    use super::*;

    #[test]
    fn bargain_offers_one_sacrifice_of_an_artifact_enchantment_or_token() {
        let extra = Bargain.additional_cost(&Keyword::Bargain).unwrap();
        assert_eq!(extra.name, "Bargain");
        assert!(!extra.repeatable);
        assert_eq!(extra.kind, OptionalKind::Additional);
        let [CostItem::Sacrifice { filter, count }] = extra.cost.0.as_slice() else {
            panic!("one sacrifice, got {:?}", extra.cost);
        };
        assert_eq!(*count, Amount::one());
        let [Pred::AnyOf(kinds)] = filter.0.as_slice() else {
            panic!("one alternative list, got {filter:?}");
        };
        assert_eq!(kinds.len(), 3);
    }

    #[rstest]
    #[case::casualty(Keyword::Casualty(2))]
    #[case::storm(Keyword::Storm)]
    #[case::conspire(Keyword::Conspire)]
    fn no_other_keyword_does(#[case] keyword: Keyword) {
        assert_eq!(Bargain.additional_cost(&keyword), None);
    }
}
