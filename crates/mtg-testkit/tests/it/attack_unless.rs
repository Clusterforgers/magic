//! Goal: `Prohibited::Attack` keeps a creature from attacking a player for whom its condition fails, offers the attack when it holds, judges "you" as the defending player and replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// Whether the Serpent of seat 0 is offered an attack on seat 1 when `unless` is its exception and `defender` is what seat 1
/// controls, and `attacker` what seat 0 controls besides it.
fn offered(
    unless: &str,
    defender: Vec<Arc<CardDefinition>>,
    attacker: Vec<Arc<CardDefinition>>,
) -> bool {
    let rule = format!(
        r#"{{"Rule":{{"rule":{{"Cant":{{"who":"This","action":{{"Attack":{{"unless":{unless}}}}}}}}}}}}}"#
    );
    let serpent = Arc::new(
        CardDefinition::new("Serpent")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(ability(&rule)),
    );
    let mut mine = vec![ready(serpent)];
    mine.extend(attacker.into_iter().map(ready));
    let scenario = Scenario::new(2)
        .at(2, 0, Step::Main1)
        .battlefield(0, mine)
        .battlefield(1, defender.into_iter().map(ready))
        .library(0, (0..10).map(|_| forest()))
        .library(1, (0..10).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    let allowed = driver.attack(&[("Serpent", 1)]).is_ok();
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    allowed
}

const ISLAND: &str = r#"{"Compare":{"left":{"Count":[{"Subtype":"Island"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#;

#[rstest]
#[case::defender_has_an_island(ISLAND, vec![island()], vec![], true)]
#[case::defender_has_only_a_forest(ISLAND, vec![forest()], vec![], false)]
#[case::defender_has_nothing(ISLAND, vec![], vec![], false)]
#[case::my_island_does_not_count(ISLAND, vec![forest()], vec![island()], false)]
#[case::negated_without_island(&format!(r#"{{"Not":{ISLAND}}}"#), vec![forest()], vec![], true)]
#[case::negated_with_island(&format!(r#"{{"Not":{ISLAND}}}"#), vec![island()], vec![], false)]
fn the_attack_is_offered_only_when_the_condition_holds_for_the_defender(
    #[case] unless: &str,
    #[case] defender: Vec<Arc<CardDefinition>>,
    #[case] attacker: Vec<Arc<CardDefinition>>,
    #[case] expected: bool,
) {
    assert_eq!(offered(unless, defender, attacker), expected);
}
