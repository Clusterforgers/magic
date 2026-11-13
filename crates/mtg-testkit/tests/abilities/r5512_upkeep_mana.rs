//! Goal: mana restricted to cumulative upkeep costs (`SpendOn::CumulativeUpkeep`) pays the cost of the cumulative upkeep
//! trigger and nothing else: Adarkar Unicorn keeps an Ager on the battlefield and cannot pay for a spell, at two to
//! four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::instant;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A 2/2 with "Cumulative upkeep {1}".
fn r5512_ager() -> Arc<CardDefinition> {
    let keyword: Ability =
        serde_json::from_str(r#"{"Keyword":{"CumulativeUpkeep":"{1}"}}"#).unwrap();
    Arc::new(
        CardDefinition::new("Ager")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(keyword),
    )
}

#[rstest]
fn r5512_unicorn_mana_pays_cumulative_upkeep(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats).battlefield(
        0,
        [ready(r5512_ager()), ready(card("Adarkar Unicorn").unwrap())],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)
        .unwrap();
    let mut asked = false;
    for _ in 0..60 {
        match &driver.decision().unwrap().kind {
            DecisionKind::YesNo(options) if !asked => {
                assert!(!options.only_no, "the Unicorn's mana can pay it");
                asked = true;
                driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.pay_tapping(&["Adarkar Unicorn"], 0).unwrap();
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    assert!(asked);
    let ager = driver.id_of("Ager").unwrap();
    assert_eq!(
        driver.state().object(ager).unwrap().zone(),
        Zone::Battlefield
    );
}

#[rstest]
fn r5512_unicorn_mana_cannot_pay_for_a_spell(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(card("Adarkar Unicorn").unwrap())])
        .hand(0, [instant("Trick", 1, &[])]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert!(driver.cast_tapping("Trick", &["Adarkar Unicorn"]).is_err());
}
