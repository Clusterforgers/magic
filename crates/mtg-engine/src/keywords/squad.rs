//! Squad (CR 702.157): "As an additional cost to cast this spell, you may pay [cost] any number of times. When this
//! creature enters, create a token that's a copy of it for each time its squad cost was paid."
//!
//! Each payment counts as a kick (`Amount::Kicks`, `Condition::Kicked`), asked and remembered as a multikicker is; the
//! token copies were not cast, so they pay nothing and make no copies of their own.

use mtg_cards::ability::Ability;
use mtg_cards::effect::{Effect, TokenRef};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Condition};

use super::{AdditionalCost, KeywordRules, OptionalKind};

/// The rules of squad.
#[derive(Debug)]
pub(super) struct Squad;

impl KeywordRules for Squad {
    fn additional_cost(&self, keyword: &Keyword) -> Option<AdditionalCost> {
        let Keyword::Squad(cost) = keyword else {
            return None;
        };
        Some(AdditionalCost {
            name: "Squad",
            cost: cost.clone(),
            repeatable: true,
            kind: OptionalKind::Kicker,
        })
    }

    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Squad(_)) {
            return Vec::new();
        }
        vec![Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector::this(),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: Some(Condition::Kicked),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::CreateToken {
                who: Ref::You,
                token: TokenRef::CopyOf {
                    of: Ref::This,
                    except: Vec::new(),
                    no_mana_cost: false,
                },
                count: Amount::Kicks,
                tapped: false,
                attacking: false,
                bind: None,
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Squad asks for its cost any number of times as an additional cost and adds an enters trigger that makes one token copy for each payment, and no other keyword does either.
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;

    fn squad(mana: &str) -> Keyword {
        Keyword::Squad(Cost::mana(mana.parse().unwrap()))
    }

    #[rstest]
    #[case::generic("{2}")]
    #[case::colored("{1}{W}")]
    fn squad_offers_its_cost_any_number_of_times(#[case] mana: &str) {
        let extra = Squad.additional_cost(&squad(mana)).unwrap();
        assert_eq!(extra.name, "Squad");
        assert!(extra.repeatable);
        assert_eq!(extra.kind, OptionalKind::Kicker);
        assert_eq!(extra.cost, Cost::mana(mana.parse().unwrap()));
    }

    #[test]
    fn squad_adds_an_enters_trigger_that_copies_it_once_for_each_payment() {
        let added = Squad.adds_abilities(&squad("{2}"));
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
        assert_eq!(*intervening_if, Some(Condition::Kicked));
        assert!(matches!(
            effects.as_slice(),
            [Effect::CreateToken {
                token: TokenRef::CopyOf { .. },
                count: Amount::Kicks,
                ..
            }]
        ));
    }

    #[rstest]
    #[case::multikicker(Keyword::Multikicker(Cost::default()))]
    #[case::myriad(Keyword::Myriad)]
    #[case::haste(Keyword::Haste)]
    fn no_other_keyword_does(#[case] keyword: Keyword) {
        assert_eq!(Squad.additional_cost(&keyword), None);
        assert_eq!(Squad.adds_abilities(&keyword), []);
    }
}
