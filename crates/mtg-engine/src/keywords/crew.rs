//! Crew (CR 702.122): "Tap any number of untapped creatures you control with total power N or more: This permanent
//! becomes an artifact creature until end of turn." The creatures are a cost, so one with summoning sickness may pay it
//! (CR 302.6), and the permanent keeps its printed power and toughness while it is a creature.

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, Modification};
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of crew.
#[derive(Debug)]
pub(super) struct Crew;

impl KeywordRules for Crew {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Crew(power) = keyword else {
            return Vec::new();
        };
        vec![Ability::Activated {
            cost: Cost(vec![CostItem::TapPower {
                filter: Selector(vec![Pred::Type(CardType::Creature)]),
                power: *power,
            }]),
            timing: Timing::Instant,
            condition: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Continuous {
                affected: Ref::This,
                mods: vec![
                    Modification::AddType(CardType::Artifact),
                    Modification::AddType(CardType::Creature),
                ],
                duration: Duration::UntilEndOfTurn,
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Crew N adds one activated ability that taps any number of untapped creatures with total power N or more and makes its source an artifact creature until end of turn, for every N, and no other keyword adds it.
    use mtg_cards::ability::Ability;
    use rstest::rstest;

    use super::*;

    fn written(power: u32) -> Ability {
        serde_json::from_str(&format!(
            r#"{{"Activated":{{"cost":[{{"TapPower":{{"filter":[{{"Type":"Creature"}}],"power":{power}}}}}],"effects":[{{"Continuous":{{"affected":"This","mods":[{{"AddType":"Artifact"}},{{"AddType":"Creature"}}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
        ))
        .unwrap()
    }

    #[rstest]
    #[case::one(1)]
    #[case::two(2)]
    #[case::three(3)]
    #[case::four(4)]
    #[case::eight(8)]
    fn crew_taps_creatures_for_their_power_and_animates_the_source(#[case] power: u32) {
        assert_eq!(Crew.adds_abilities(&Keyword::Crew(power)), [written(power)]);
    }

    #[rstest]
    #[case::flying(Keyword::Flying)]
    #[case::equip(Keyword::Equip(Cost::mana("{1}".parse().unwrap())))]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Crew.adds_abilities(&keyword), []);
    }
}
