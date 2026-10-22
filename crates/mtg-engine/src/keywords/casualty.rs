//! Casualty N (CR 702.153): "As an additional cost to cast this spell, you may sacrifice a creature with power N or
//! greater. When you cast this spell, if a casualty cost was paid for it, copy it. If the spell has targets, you may
//! choose new targets for the copy."
//!
//! The cost is an optional additional cost read as `Condition::AdditionalCostPaid`; the trigger works from the stack
//! like storm's, and the copy is not cast, so it has no casualty trigger of its own.

use mtg_cards::ability::Ability;
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Cmp, Condition, StatKind};
use mtg_core::card::CardType;

use super::{AdditionalCost, KeywordRules, OptionalKind};

/// The rules of casualty.
#[derive(Debug)]
pub(super) struct Casualty;

impl KeywordRules for Casualty {
    fn additional_cost(&self, keyword: &Keyword) -> Option<AdditionalCost> {
        let Keyword::Casualty(power) = keyword else {
            return None;
        };
        let power = i32::try_from(*power).unwrap_or(i32::MAX);
        Some(AdditionalCost {
            name: "Casualty",
            cost: Cost(vec![CostItem::Sacrifice {
                filter: Selector(vec![
                    Pred::Type(CardType::Creature),
                    Pred::Compare {
                        stat: StatKind::Power,
                        op: Cmp::Ge,
                        to: Amount::Fixed(power),
                    },
                ]),
                count: Amount::one(),
            }]),
            repeatable: false,
            kind: OptionalKind::Additional,
        })
    }

    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Casualty(_)) {
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
    //! Goal: Casualty offers one optional cost that sacrifices a creature of power N or more and adds a cast trigger that copies the spell once if it was paid, and no other keyword does either.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::one(1)]
    #[case::two(2)]
    #[case::three(3)]
    fn casualty_offers_a_sacrifice_of_power_n_or_more_once(#[case] power: u32) {
        let extra = Casualty.additional_cost(&Keyword::Casualty(power)).unwrap();
        assert_eq!(extra.name, "Casualty");
        assert!(!extra.repeatable);
        assert_eq!(extra.kind, OptionalKind::Additional);
        let [CostItem::Sacrifice { filter, count }] = extra.cost.0.as_slice() else {
            panic!("one sacrifice, got {:?}", extra.cost);
        };
        assert_eq!(*count, Amount::one());
        assert!(filter.0.contains(&Pred::Compare {
            stat: StatKind::Power,
            op: Cmp::Ge,
            to: Amount::Fixed(i32::try_from(power).unwrap()),
        }));
    }

    #[test]
    fn casualty_adds_a_cast_trigger_that_copies_the_spell_if_the_cost_was_paid() {
        let added = Casualty.adds_abilities(&Keyword::Casualty(2));
        let [
            Ability::Triggered {
                when: TriggerPattern::SpellCast { .. },
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
    #[case::squad(Keyword::Squad(Cost::default()))]
    #[case::storm(Keyword::Storm)]
    #[case::offspring(Keyword::Offspring(Cost::default()))]
    fn no_other_keyword_does(#[case] keyword: Keyword) {
        assert_eq!(Casualty.additional_cost(&keyword), None);
        assert_eq!(Casualty.adds_abilities(&keyword), []);
    }
}
