//! Goal: a block capacity counted by an amount (`BlocksAdditionalPer`, "can block an additional creature each combat for each Equipment attached to this creature") lets the creature block one more attacker for each Equipment attached to it now and no more, at two to four seats and with none, one or two attached: blocking one attacker more than that is refused, and the log of a legal block replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const LEGION: &str = r#"{"Static":{"affected":"This","mods":[{"BlocksAdditionalPer":{"Count":[{"Subtype":"Equipment"},{"AttachedTo":"This"}]}}]}}"#;

fn legion() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(LEGION).unwrap();
    Arc::new(
        CardDefinition::new("Legion")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 9)
            .with_rules(rule),
    )
}

fn gear(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_subtype("Equipment"),
    )
}

const ATTACKERS: [&str; 4] = ["A1", "A2", "A3", "A4"];

#[rstest]
fn rmt7_the_creature_blocks_one_more_for_each_equipment(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] attached: usize,
    #[values(false, true)] one_too_many: bool,
) {
    let mut defender = vec![ready(legion())];
    for index in 0..attached {
        let mut equipment = ready(gear(&format!("Gear {index}")));
        equipment.attached_to = Some((1, 0));
        defender.push(equipment);
    }
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, ATTACKERS.map(|name| ready(free_creature(name, 1, 1))))
        .battlefield(1, defender);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let attacks: Vec<(&str, usize)> = ATTACKERS.iter().map(|name| (*name, 1)).collect();
    driver.attack(&attacks).unwrap();
    driver.pass_until(Step::DeclareBlockers).unwrap();
    let blocked = attached + 1 + usize::from(one_too_many);
    let blocks: Vec<(&str, &str)> = ATTACKERS[..blocked]
        .iter()
        .map(|name| ("Legion", *name))
        .collect();
    let answer = driver.block(&blocks);
    if one_too_many {
        assert!(answer.is_err(), "one attacker more than its capacity");
        return;
    }
    answer.unwrap();
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
