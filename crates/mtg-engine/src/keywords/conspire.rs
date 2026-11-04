//! Conspire (CR 702.78): "As an additional cost to cast this spell, you may tap two untapped creatures you control that
//! share a color with it. When you cast this spell, if its conspire cost was paid, copy it. If the spell has targets,
//! you may choose new targets for the copy."
//!
//! The cost is an optional additional cost read as `Condition::AdditionalCostPaid`; the trigger works from the stack
//! like casualty's, and the copy is not cast, so it has no conspire trigger of its own.

use mtg_cards::ability::Ability;
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Condition};
use mtg_core::card::CardType;

use super::{AdditionalCost, KeywordRules, OptionalKind};

/// The rules of conspire.
#[derive(Debug)]
pub(super) struct Conspire;

impl KeywordRules for Conspire {
    fn additional_cost(&self, keyword: &Keyword) -> Option<AdditionalCost> {
        if !matches!(keyword, Keyword::Conspire) {
            return None;
        }
        Some(AdditionalCost {
            name: "Conspire",
            cost: Cost(vec![CostItem::TapPermanents {
                filter: Selector(vec![
                    Pred::Type(CardType::Creature),
                    Pred::Untapped,
                    Pred::SharesColor(Ref::This),
                ]),
                count: Amount::Fixed(2),
            }]),
            repeatable: false,
            kind: OptionalKind::Additional,
        })
    }

    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Conspire) {
            return Vec::new();
        }
        vec![Ability::Triggered {
            when: TriggerPattern::SpellCast {
                what: Selector::this(),
                by: Ref::You,
            },
            intervening_if: Some(Condition::AdditionalCostPaid),
            limit: None,
            from: ZoneKind::Stack,
            targets: Vec::new(),
            effects: vec![Effect::CopySpell {
                who: Ref::You,
                what: Ref::This,
                count: Amount::one(),
                new_targets: true,
                except: Vec::new(),
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Conspire offers one optional cost that taps two creatures sharing a color with the spell and adds a cast trigger that copies the spell once if it was paid, and no other keyword does either.
    use rstest::rstest;

    use super::*;

    #[test]
    fn conspire_offers_a_tap_of_two_creatures_of_the_color_of_the_spell() {
        let extra = Conspire.additional_cost(&Keyword::Conspire).unwrap();
        assert_eq!(extra.name, "Conspire");
        assert!(!extra.repeatable);
        assert_eq!(extra.kind, OptionalKind::Additional);
        let [CostItem::TapPermanents { filter, count }] = extra.cost.0.as_slice() else {
            panic!("one tap, got {:?}", extra.cost);
        };
        assert_eq!(*count, Amount::Fixed(2));
        assert!(filter.0.contains(&Pred::SharesColor(Ref::This)));
    }

    #[test]
    fn conspire_adds_a_cast_trigger_that_copies_the_spell_if_the_cost_was_paid() {
        let added = Conspire.adds_abilities(&Keyword::Conspire);
        let [
            Ability::Triggered {
                intervening_if,
                from,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one cast trigger, got {added:?}");
        };
        assert_eq!(*intervening_if, Some(Condition::AdditionalCostPaid));
        assert_eq!(*from, ZoneKind::Stack);
        assert!(matches!(effects.as_slice(), [Effect::CopySpell { .. }]));
    }

    #[rstest]
    #[case::casualty(Keyword::Casualty(2))]
    #[case::storm(Keyword::Storm)]
    #[case::bargain(Keyword::Bargain)]
    fn no_other_keyword_does(#[case] keyword: Keyword) {
        assert_eq!(Conspire.additional_cost(&keyword), None);
        assert_eq!(Conspire.adds_abilities(&keyword), []);
    }
}
