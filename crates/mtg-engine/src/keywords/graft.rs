//! Graft (CR 702.58): "This permanent enters with N +1/+1 counters on it. Whenever another creature enters, if this
//! permanent has a +1/+1 counter on it, you may move a +1/+1 counter from this permanent onto that creature."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Cmp, CounterKind};
use mtg_core::card::CardType;

use super::KeywordRules;
use super::vanishing::{counters_are, enters_with};

/// The rules of graft.
#[derive(Debug)]
pub(super) struct Graft;

impl KeywordRules for Graft {
    /// The replacement puts the counters on as the permanent enters; each instance has its own trigger, which asks for a
    /// counter on the permanent when it is put on the stack and as it resolves (the counter may have left in between).
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Graft(count) = keyword else {
            return Vec::new();
        };
        let counter = CounterKind::PlusOnePlusOne;
        let mut abilities: Vec<Ability> =
            enters_with(counter.clone(), *count).into_iter().collect();
        abilities.push(Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector(vec![Pred::Other, Pred::Type(CardType::Creature)]),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: Some(counters_are(&counter, Cmp::Ge, 1)),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::May {
                who: Ref::You,
                effects: vec![Effect::MoveCounters {
                    from: Ref::This,
                    to: Ref::Triggering,
                    counter,
                    count: Amount::one(),
                }],
            }],
        });
        abilities
    }
}

#[cfg(test)]
mod tests {
    //! Goal: graft adds the entering replacement for its count and one optional move trigger, and none for another keyword.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::one(1, 2)]
    #[case::three(3, 2)]
    #[case::none(0, 1)]
    fn graft_adds_a_replacement_and_a_trigger(#[case] count: u32, #[case] abilities: usize) {
        assert_eq!(
            Graft.adds_abilities(&Keyword::Graft(count)).len(),
            abilities
        );
    }

    #[test]
    fn another_keyword_adds_nothing() {
        assert_eq!(Graft.adds_abilities(&Keyword::Riot), Vec::new());
    }
}
