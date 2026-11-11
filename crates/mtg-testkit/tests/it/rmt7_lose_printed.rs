//! Goal: an ability any player may activate (`Activators::Anyone`) that makes its source lose one of its printed abilities until end of turn (`LosePrinted`, Glittering Lion: "{3}: Until end of turn, this creature loses \"Prevent all damage that would be dealt to this creature.\" Any player may activate this ability.") is offered to an opponent, takes away only that ability and lets damage through, at two to four seats; without the activation the damage is prevented, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const PREVENT: &str = r#"{"Replacement":{"when":{"Damage":{"to":"This"}},"by":"Prevent"}}"#;
const LOSE: &str = r#"{"Activated":{"cost":"{3}","by":"Anyone","effects":[{"Continuous":{"affected":"This","mods":[{"LosePrinted":{"Replacement":{"when":{"Damage":{"to":"This"}},"by":"Prevent"}}}],"duration":"UntilEndOfTurn"}}]}}"#;

fn lion() -> Arc<CardDefinition> {
    let prevent: Ability = serde_json::from_str(PREVENT).unwrap();
    let lose: Ability = serde_json::from_str(LOSE).unwrap();
    Arc::new(
        CardDefinition::new("Lion")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(prevent)
            .with_rules(lose),
    )
}

fn settle(driver: &mut Driver) {
    for _ in 0..50 {
        if driver.state().stack().is_empty() {
            return;
        }
        if matches!(driver.decision().unwrap().kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
        } else {
            driver.pass().unwrap();
        }
    }
    panic!("the stack does not empty");
}

fn alive(driver: &Driver) -> bool {
    driver
        .id_of("Lion")
        .is_ok_and(|id| driver.state().zone(Zone::Battlefield).contains(&id))
}

#[rstest]
fn rmt7_an_opponent_takes_away_the_prevention_until_end_of_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] activated: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [(); 4].map(|()| ready(mountain())))
        .battlefield(
            1,
            [
                ready(lion()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand(0, [burn("Bolt", 3), burn("Shock", 2)]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    if activated {
        let lion = driver.target_named("Lion").unwrap();
        // The active player is an opponent of the Lion's controller and activates it.
        assert_eq!(driver.offered_abilities("Lion").unwrap(), vec![1]);
        driver.begin_ability("Lion", 1).unwrap();
        driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
        settle(&mut driver);
        driver.cast_at("Shock", &[lion]).unwrap();
        settle(&mut driver);
        assert!(!alive(&driver), "the damage is no longer prevented");
    } else {
        let lion = driver.target_named("Lion").unwrap();
        driver.cast_at("Bolt", &[lion]).unwrap();
        settle(&mut driver);
        assert!(alive(&driver), "the damage is prevented");
    }
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
