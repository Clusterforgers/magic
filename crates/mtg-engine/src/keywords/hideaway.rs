//! Hideaway N (CR 702.75a): "When this permanent enters, look at the top N cards of your library. Exile one of them face
//! down and put the rest on the bottom of your library in a random order."
//!
//! The keyword adds an enters trigger with the steps of `effect::hideaway`. The exiled card lies face down in exile
//! linked to the permanent (`FaceDown::Hideaway`): its owner and the controller of the permanent may look at it, and the
//! abilities of the permanent find it with `Pred::ExiledWith`. The permanent leaving the battlefield ends the link; the
//! card stays in exile and can no longer be played. Each instance of the keyword is its own trigger.

use mtg_cards::ability::Ability;
use mtg_cards::effect::hideaway;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;

use super::KeywordRules;

/// The rules of hideaway.
#[derive(Debug)]
pub(super) struct Hideaway;

impl KeywordRules for Hideaway {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Hideaway(count) = keyword else {
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
            effects: hideaway(Amount::Fixed(count)),
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: hideaway adds one enters trigger of the source that looks at the top N cards, chooses one, hides it and puts the rest on the bottom at random, and no other keyword adds it.
    use mtg_cards::effect::Effect;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::three(3)]
    #[case::four(4)]
    #[case::five(5)]
    fn hideaway_adds_an_enters_trigger_that_looks_at_n_cards(#[case] number: u32) {
        let added = Hideaway.adds_abilities(&Keyword::Hideaway(number));
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
        let Some(Effect::LookAtTop { count, .. }) = effects.first() else {
            panic!("a look first: {effects:?}");
        };
        assert_eq!(*count, Amount::Fixed(i32::try_from(number).unwrap()));
        assert!(matches!(
            effects.as_slice(),
            [
                Effect::LookAtTop { .. },
                Effect::Choose { .. },
                Effect::ExileHidden { .. },
                Effect::Move { .. }
            ]
        ));
    }

    #[rstest]
    #[case::demonstrate(Keyword::Demonstrate)]
    #[case::fabricate(Keyword::Fabricate(2))]
    fn no_other_keyword_does(#[case] keyword: Keyword) {
        assert_eq!(Hideaway.adds_abilities(&keyword), []);
    }
}
