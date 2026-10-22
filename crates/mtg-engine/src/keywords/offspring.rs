//! Offspring (CR 702.175): "You may pay an additional [cost] as you cast this spell. If you do, when this creature
//! enters, create a 1/1 token copy of it."
//!
//! The cost is an optional additional cost read as `Condition::AdditionalCostPaid`; the enters trigger makes the copy
//! with its power and toughness set to 1 as part of its copiable values. The token was not cast, so it paid nothing and
//! makes no copy of its own.

use mtg_cards::ability::Ability;
use mtg_cards::effect::{Effect, TokenRef};
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Modification;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Condition};

use super::{AdditionalCost, KeywordRules, OptionalKind};

/// The rules of offspring.
#[derive(Debug)]
pub(super) struct Offspring;

impl KeywordRules for Offspring {
    fn additional_cost(&self, keyword: &Keyword) -> Option<AdditionalCost> {
        let Keyword::Offspring(cost) = keyword else {
            return None;
        };
        Some(AdditionalCost {
            name: "Offspring",
            cost: cost.clone(),
            repeatable: false,
            kind: OptionalKind::Additional,
        })
    }

    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Offspring(_)) {
            return Vec::new();
        }
        vec![Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector::this(),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: Some(Condition::AdditionalCostPaid),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::CreateToken {
                who: Ref::You,
                token: TokenRef::CopyOf {
                    of: Ref::This,
                    except: vec![Modification::SetPT {
                        power: Amount::Fixed(1),
                        toughness: Amount::Fixed(1),
                    }],
                    no_mana_cost: false,
                },
                count: Amount::one(),
                tapped: false,
                attacking: false,
                bind: None,
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Offspring offers its cost once and adds an enters trigger that makes one 1/1 token copy if it was paid, and no other keyword does either.
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;

    fn offspring(mana: &str) -> Keyword {
        Keyword::Offspring(Cost::mana(mana.parse().unwrap()))
    }

    #[rstest]
    #[case::generic("{2}")]
    #[case::colored("{1}{U}")]
    fn offspring_offers_its_cost_once(#[case] mana: &str) {
        let extra = Offspring.additional_cost(&offspring(mana)).unwrap();
        assert_eq!(extra.name, "Offspring");
        assert!(!extra.repeatable);
        assert_eq!(extra.kind, OptionalKind::Additional);
        assert_eq!(extra.cost, Cost::mana(mana.parse().unwrap()));
    }

    #[test]
    fn offspring_adds_an_enters_trigger_that_makes_a_one_one_copy_if_paid() {
        let added = Offspring.adds_abilities(&offspring("{2}"));
        let [
            Ability::Triggered {
                intervening_if,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one trigger, got {added:?}");
        };
        assert_eq!(*intervening_if, Some(Condition::AdditionalCostPaid));
        let [
            Effect::CreateToken {
                token: TokenRef::CopyOf { except, .. },
                count,
                ..
            },
        ] = effects.as_slice()
        else {
            panic!("one token copy, got {effects:?}");
        };
        assert_eq!(*count, Amount::one());
        assert_eq!(
            *except,
            [Modification::SetPT {
                power: Amount::Fixed(1),
                toughness: Amount::Fixed(1)
            }]
        );
    }

    #[rstest]
    #[case::squad(Keyword::Squad(Cost::default()))]
    #[case::casualty(Keyword::Casualty(1))]
    #[case::kicker(Keyword::Kicker(Cost::default()))]
    fn no_other_keyword_does(#[case] keyword: Keyword) {
        assert_eq!(Offspring.additional_cost(&keyword), None);
        assert_eq!(Offspring.adds_abilities(&keyword), []);
    }
}
