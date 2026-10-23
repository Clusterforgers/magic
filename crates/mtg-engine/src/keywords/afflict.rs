//! Afflict (CR 702.130): "Whenever this creature becomes blocked, defending player loses N life."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;

use super::KeywordRules;

/// The rules of afflict.
#[derive(Debug)]
pub(super) struct Afflict;

impl KeywordRules for Afflict {
    /// One trigger for each instance; the player is the one the creature attacks, still named once it has left combat.
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Afflict(count) = keyword else {
            return Vec::new();
        };
        vec![Ability::Triggered {
            when: TriggerPattern::BecomesBlocked {
                what: Selector::this(),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::LoseLife {
                who: Ref::Defending,
                amount: Amount::Fixed(i32::try_from(*count).unwrap_or(i32::MAX)),
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: afflict adds one trigger that makes the defending player lose its count of life, and nothing for another keyword.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::one(1)]
    #[case::three(3)]
    fn afflict_makes_the_defending_player_lose_its_count(#[case] count: u32) {
        let added = Afflict.adds_abilities(&Keyword::Afflict(count));
        let [Ability::Triggered { effects, .. }] = added.as_slice() else {
            panic!("one trigger expected");
        };
        let [Effect::LoseLife { who, amount }] = effects.as_slice() else {
            panic!("one loss expected");
        };
        assert_eq!(*who, Ref::Defending);
        assert_eq!(*amount, Amount::Fixed(i32::try_from(count).unwrap()));
    }

    #[test]
    fn another_keyword_adds_nothing() {
        assert_eq!(Afflict.adds_abilities(&Keyword::Graft(1)), Vec::new());
    }
}
