//! Fabricate (CR 702.123): "When this permanent enters, you may put N +1/+1 counters on it. If you don't, create N
//! 1/1 colorless Servo artifact creature tokens."
//!
//! The keyword adds an enters trigger whose effect is `Effect::Fabricate`, which asks the choice.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;

use super::KeywordRules;

/// The rules of fabricate.
#[derive(Debug)]
pub(super) struct Fabricate;

impl KeywordRules for Fabricate {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Fabricate(count) = keyword else {
            return Vec::new();
        };
        let count = i32::try_from(*count).unwrap_or(i32::MAX);
        vec![Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector::this(),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Fabricate {
                count: Amount::Fixed(count),
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: fabricate adds one enters trigger of the source whose effect fabricates the number, and no other keyword adds it.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::one(1)]
    #[case::two(2)]
    #[case::three(3)]
    fn fabricate_adds_an_enters_trigger_with_its_number(#[case] number: u32) {
        let added = Fabricate.adds_abilities(&Keyword::Fabricate(number));
        let [Ability::Triggered { when, effects, .. }] = added.as_slice() else {
            panic!("one trigger, got {added:?}");
        };
        assert!(matches!(
            when,
            TriggerPattern::ZoneChange {
                to: Some(ZoneKind::Battlefield),
                ..
            }
        ));
        assert_eq!(
            effects,
            &[Effect::Fabricate {
                count: Amount::Fixed(i32::try_from(number).unwrap())
            }]
        );
    }

    #[rstest]
    #[case::ravenous(Keyword::Ravenous)]
    #[case::ascend(Keyword::Ascend)]
    #[case::haste(Keyword::Haste)]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Fabricate.adds_abilities(&keyword), []);
    }
}
