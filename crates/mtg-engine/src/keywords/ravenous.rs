//! Ravenous (CR 702.156): "This creature enters with X +1/+1 counters on it. If X is 5 or more, draw a card when it
//! enters."
//!
//! The counters are an `Enters` replacement that reads the X the spell was cast with; the draw is an enters trigger
//! whose intervening "if" reads the same X (the permanent remembers it, CR 107.3m).

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use mtg_cards::value::{Amount, Cmp, Condition, CounterKind};

use super::KeywordRules;

/// The least X that draws a card.
const DRAWS_FROM: i32 = 5;

/// The rules of ravenous.
#[derive(Debug)]
pub(super) struct Ravenous;

impl KeywordRules for Ravenous {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Ravenous) {
            return Vec::new();
        }
        vec![
            Ability::Replacement {
                when: ReplaceWhen::Enters {
                    what: Selector::this(),
                },
                by: ReplaceBy::WithCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::X,
                },
                condition: None,
                from: ZoneKind::Battlefield,
            },
            Ability::Triggered {
                when: TriggerPattern::ZoneChange {
                    what: Selector::this(),
                    from: None,
                    to: Some(ZoneKind::Battlefield),
                },
                intervening_if: Some(Condition::Compare {
                    left: Amount::X,
                    op: Cmp::Ge,
                    right: Amount::Fixed(DRAWS_FROM),
                }),
                limit: None,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: vec![Effect::Draw {
                    who: Ref::You,
                    count: Amount::one(),
                }],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Ravenous adds an enters replacement with X +1/+1 counters and an enters trigger that draws a card when X is 5 or more, and no other keyword adds them.
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;

    #[test]
    fn ravenous_adds_the_counters_and_the_draw() {
        let added = Ravenous.adds_abilities(&Keyword::Ravenous);
        let [
            Ability::Replacement { by, .. },
            Ability::Triggered {
                intervening_if,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("a replacement and a trigger, got {added:?}");
        };
        assert_eq!(
            *by,
            ReplaceBy::WithCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::X
            }
        );
        assert_eq!(
            *intervening_if,
            Some(Condition::Compare {
                left: Amount::X,
                op: Cmp::Ge,
                right: Amount::Fixed(5)
            })
        );
        assert!(matches!(effects.as_slice(), [Effect::Draw { .. }]));
    }

    #[rstest]
    #[case::squad(Keyword::Squad(Cost::default()))]
    #[case::undying(Keyword::Undying)]
    #[case::haste(Keyword::Haste)]
    fn no_other_keyword_adds_them(#[case] keyword: Keyword) {
        assert_eq!(Ravenous.adds_abilities(&keyword), []);
    }
}
