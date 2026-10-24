//! Replicate (CR 702.56): "As an additional cost to cast this spell, you may pay [cost] any number of times. When you
//! cast this spell, copy it for each time its replicate cost was paid. You may choose new targets for the copies."
//!
//! Each payment counts as a kick (`Amount::Kicks`, `Condition::Kicked`), asked and remembered as a multikicker is; the
//! trigger works from the stack like storm's, and a copy is not cast, so it pays nothing and replicates no further.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Condition};

use super::{AdditionalCost, KeywordRules, OptionalKind};

/// The rules of replicate.
#[derive(Debug)]
pub(super) struct Replicate;

impl KeywordRules for Replicate {
    fn additional_cost(&self, keyword: &Keyword) -> Option<AdditionalCost> {
        let Keyword::Replicate(cost) = keyword else {
            return None;
        };
        Some(AdditionalCost {
            name: "Replicate",
            cost: cost.clone(),
            repeatable: true,
            kind: OptionalKind::Kicker,
        })
    }

    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Replicate(_)) {
            return Vec::new();
        }
        vec![Ability::Triggered {
            when: TriggerPattern::SpellCast {
                what: Selector::this(),
                by: Ref::You,
            },
            intervening_if: Some(Condition::Kicked),
            limit: None,
            from: ZoneKind::Stack,
            targets: Vec::new(),
            effects: vec![Effect::CopySpell {
                who: Ref::You,
                what: Ref::This,
                count: Amount::Kicks,
                new_targets: true,
                except: Vec::new(),
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Replicate asks for its cost any number of times as an additional cost and adds a cast trigger that copies the spell once for each payment, and no other keyword does either.
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;

    fn replicate(mana: &str) -> Keyword {
        Keyword::Replicate(Cost::mana(mana.parse().unwrap()))
    }

    #[rstest]
    #[case::generic("{2}")]
    #[case::colored("{1}{G}")]
    fn replicate_offers_its_cost_any_number_of_times(#[case] mana: &str) {
        let extra = Replicate.additional_cost(&replicate(mana)).unwrap();
        assert_eq!(extra.name, "Replicate");
        assert!(extra.repeatable);
        assert_eq!(extra.kind, OptionalKind::Kicker);
        assert_eq!(extra.cost, Cost::mana(mana.parse().unwrap()));
    }

    #[test]
    fn replicate_adds_a_cast_trigger_that_copies_the_spell_once_for_each_payment() {
        let added = Replicate.adds_abilities(&replicate("{2}"));
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
        assert_eq!(*intervening_if, Some(Condition::Kicked));
        assert_eq!(*from, ZoneKind::Stack);
        let [Effect::CopySpell { count, .. }] = effects.as_slice() else {
            panic!("one copy, got {effects:?}");
        };
        assert_eq!(*count, Amount::Kicks);
    }

    #[rstest]
    #[case::squad(Keyword::Squad(Cost::default()))]
    #[case::storm(Keyword::Storm)]
    #[case::casualty(Keyword::Casualty(1))]
    fn no_other_keyword_does(#[case] keyword: Keyword) {
        assert_eq!(Replicate.additional_cost(&keyword), None);
        assert_eq!(Replicate.adds_abilities(&keyword), []);
    }
}
