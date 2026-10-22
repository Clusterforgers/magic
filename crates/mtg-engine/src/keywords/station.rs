//! Station (CR 702.184): "Tap another untapped creature you control: Put charge counters equal to its power on this
//! permanent. Activate only as a sorcery." The creature is a cost, so one with summoning sickness may pay it (CR 302.6).
//! The STATION brackets of the card are static and activated abilities conditioned on the charge counters
//! (`docs/cards-dsl.md`).

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::value::{Amount, CounterKind, StatKind};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of station.
#[derive(Debug)]
pub(super) struct Station;

impl KeywordRules for Station {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if *keyword != Keyword::Station {
            return Vec::new();
        }
        vec![Ability::Activated {
            cost: Cost(vec![CostItem::TapPermanents {
                filter: Selector(vec![Pred::Type(CardType::Creature), Pred::Other]),
                count: Amount::one(),
            }]),
            timing: Timing::Sorcery,
            condition: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::PutCounters {
                on: Ref::This,
                counter: CounterKind::Charge,
                count: Amount::Stat {
                    of: Ref::Paid,
                    stat: StatKind::Power,
                },
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Station adds one sorcery-speed activated ability that taps another creature and puts charge counters equal to its power on the source, and no other keyword adds it.
    use mtg_cards::ability::Ability;
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;

    fn written() -> Ability {
        serde_json::from_str(
            r#"{"Activated":{"cost":[{"TapPermanents":{"filter":[{"Type":"Creature"},"Other"]}}],"timing":"Sorcery","effects":[{"PutCounters":{"on":"This","counter":"Charge","count":{"Stat":{"of":"Paid","stat":"Power"}}}}]}}"#,
        )
        .unwrap()
    }

    #[test]
    fn station_taps_another_creature_for_charge_counters_equal_to_its_power() {
        assert_eq!(Station.adds_abilities(&Keyword::Station), [written()]);
    }

    #[rstest]
    #[case::flying(Keyword::Flying)]
    #[case::crew(Keyword::Crew(2))]
    #[case::saddle(Keyword::Saddle(2))]
    #[case::level_up(Keyword::LevelUp(Cost::mana("{1}".parse().unwrap())))]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Station.adds_abilities(&keyword), []);
    }
}
