//! Saddle (CR 702.171): "Tap any number of other untapped creatures you control with total power N or more: This
//! Mount becomes saddled until end of turn. Activate only as a sorcery." The creatures are a cost, so one with summoning
//! sickness may pay it (CR 302.6). Saddled is the designation a Mount holds until end of turn; the effect grants it as
//! `Keyword::Saddled` and cards read it with `HasKeyword`.

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, Modification};
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of saddle.
#[derive(Debug)]
pub(super) struct Saddle;

impl KeywordRules for Saddle {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Saddle(power) = keyword else {
            return Vec::new();
        };
        vec![Ability::Activated {
            cost: Cost(vec![CostItem::TapPower {
                filter: Selector(vec![Pred::Type(CardType::Creature), Pred::Other]),
                power: *power,
            }]),
            timing: Timing::Sorcery,
            condition: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Continuous {
                affected: Ref::This,
                mods: vec![Modification::AddKeyword(Keyword::Saddled)],
                duration: Duration::UntilEndOfTurn,
            }],
        }]
    }
}

/// The saddled designation: it has no rules of its own, cards read it.
#[derive(Debug)]
pub(super) struct Saddled;

impl KeywordRules for Saddled {}

#[cfg(test)]
mod tests {
    //! Goal: Saddle N adds one sorcery-speed activated ability that taps other creatures with total power N or more and makes its source saddled until end of turn, for every N, and no other keyword adds it.
    use mtg_cards::ability::Ability;
    use rstest::rstest;

    use super::*;

    fn written(power: u32) -> Ability {
        serde_json::from_str(&format!(
            r#"{{"Activated":{{"cost":[{{"TapPower":{{"filter":[{{"Type":"Creature"}},"Other"],"power":{power}}}}}],"timing":"Sorcery","effects":[{{"Continuous":{{"affected":"This","mods":[{{"AddKeyword":"Saddled"}}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
        ))
        .unwrap()
    }

    #[rstest]
    #[case::one(1)]
    #[case::two(2)]
    #[case::three(3)]
    #[case::four(4)]
    #[case::nine(9)]
    fn saddle_taps_other_creatures_for_their_power_and_marks_the_source(#[case] power: u32) {
        assert_eq!(
            Saddle.adds_abilities(&Keyword::Saddle(power)),
            [written(power)]
        );
    }

    #[rstest]
    #[case::flying(Keyword::Flying)]
    #[case::crew(Keyword::Crew(2))]
    #[case::saddled(Keyword::Saddled)]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Saddle.adds_abilities(&keyword), []);
        assert_eq!(Saddled.adds_abilities(&keyword), []);
    }
}
