//! Sunburst (CR 702.44): "This enters with a +1/+1 counter on it for each color of mana spent to cast it" (a charge counter
//! unless it is a creature).
//!
//! Two `Enters` replacements, one for creatures and one for the rest, that read the colors among the mana the spell was cast
//! with (`Amount::ColorsSpent`); a permanent put onto the battlefield without being cast enters with none.

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_cards::value::{Amount, Condition, CounterKind};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of sunburst.
#[derive(Debug)]
pub(super) struct Sunburst;

impl KeywordRules for Sunburst {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Sunburst) {
            return Vec::new();
        }
        let creature = Condition::Matches {
            subject: Ref::This,
            selector: Selector(vec![Pred::Type(CardType::Creature)]),
        };
        vec![
            entering(CounterKind::PlusOnePlusOne, creature.clone()),
            entering(CounterKind::Charge, Condition::Not(Box::new(creature))),
        ]
    }
}

/// The replacement that gives the permanent a counter of `counter` for each color spent when `condition` holds.
fn entering(counter: CounterKind, condition: Condition) -> Ability {
    Ability::Replacement {
        when: ReplaceWhen::Enters {
            what: Selector::this(),
        },
        by: ReplaceBy::WithCounters {
            counter,
            count: Amount::ColorsSpent,
            tapped: false,
        },
        condition: Some(condition),
        from: ZoneKind::Battlefield,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: sunburst adds two enters replacements, +1/+1 counters for a creature and charge counters for any other permanent, each as many as the colors of mana spent; no other keyword adds them.
    use super::*;

    #[test]
    fn sunburst_adds_the_counters_by_type() {
        let added = Sunburst.adds_abilities(&Keyword::Sunburst);
        let kinds: Vec<_> = added
            .iter()
            .map(|ability| match ability {
                Ability::Replacement {
                    by:
                        ReplaceBy::WithCounters {
                            counter,
                            count: Amount::ColorsSpent,
                            ..
                        },
                    condition: Some(_),
                    ..
                } => counter.clone(),
                other => panic!("an enters replacement: {other:?}"),
            })
            .collect();
        assert_eq!(kinds, [CounterKind::PlusOnePlusOne, CounterKind::Charge]);
    }

    #[test]
    fn sunburst_adds_nothing_for_another_keyword() {
        assert_eq!(Sunburst.adds_abilities(&Keyword::Ravenous), Vec::new());
    }
}
