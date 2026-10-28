//! Renown (CR 702.112): "When this creature deals combat damage to a player, if it isn't renowned, put N +1/+1
//! counters on it and it becomes renowned."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Condition};

use super::KeywordRules;

/// The rules of renown.
#[derive(Debug)]
pub(super) struct Renown;

impl KeywordRules for Renown {
    /// One trigger for each instance (CR 702.112b); the intervening "if" holds back each once the creature is renowned.
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Renown(count) = keyword else {
            return Vec::new();
        };
        vec![Ability::Triggered {
            when: TriggerPattern::DealsDamage {
                source: Selector::this(),
                to: Some(Ref::EachPlayer),
                combat: Some(true),
            },
            intervening_if: Some(Condition::Not(Box::new(Condition::Matches {
                subject: Ref::This,
                selector: Selector(vec![Pred::Renowned]),
            }))),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Renown {
                count: Amount::Fixed(i32::try_from(*count).unwrap_or(i32::MAX)),
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: renown adds one combat damage trigger, held back once the creature is renowned, that renowns it with its count, and nothing for another keyword.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::one(1)]
    #[case::two(2)]
    fn renown_renowns_the_creature_with_its_count(#[case] count: u32) {
        let added = Renown.adds_abilities(&Keyword::Renown(count));
        let [
            Ability::Triggered {
                when,
                intervening_if: Some(Condition::Not(_)),
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one trigger with an intervening if expected");
        };
        assert!(matches!(
            when,
            TriggerPattern::DealsDamage {
                combat: Some(true),
                ..
            }
        ));
        assert_eq!(
            effects.as_slice(),
            [Effect::Renown {
                count: Amount::Fixed(i32::try_from(count).unwrap())
            }]
        );
    }

    #[test]
    fn another_keyword_adds_nothing() {
        assert_eq!(Renown.adds_abilities(&Keyword::Exploit), Vec::new());
    }
}
