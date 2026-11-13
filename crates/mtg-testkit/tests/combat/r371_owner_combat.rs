//! Goal: combat measured from a creature's owner, at 2 to 4 seats, for a creature seat 0 controls and seat 1 owns: "can't attack its owner" refuses the attack on the owner and allows one on another player, "can't be blocked unless it's attacking its owner or a permanent its owner controls" lets only the owner block, and "whenever a creature deals combat damage to its owner" triggers on that damage alone; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn creature(name: &str, rules: &[&str]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    for rule in rules {
        card = card.with_rules(ability(rule));
    }
    Arc::new(card)
}

/// The Raider changes control as the game begins, so it attacks with haste.
const HASTE: &str = r#"{"Keyword":"Haste"}"#;
/// Seat 0 controls the Raider its opponent owns.
const THIEF: &str = r#"{"Static":{"affected":{"Each":[{"Named":"Raider"},{"OwnedBy":"Opponents"}]},"mods":[{"SetController":"You"}]}}"#;
/// Seat 0's life rises by 1 whenever a creature deals combat damage to its owner.
const WATCHER: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Type":"Creature"}],"to":{"OwnerOf":"Dealer"},"combat":true}},"effects":[{"GainLife":{"amount":1}}]}}"#;

/// Seat 0 holds the Thief and the Watcher, seat 1 owns the Raider with `rules` and haste, each other seat has a Wall, and every seat has a library.
fn table(seats: usize, rules: &[&str]) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(creature("Thief", &[THIEF, WATCHER]))])
        .battlefield(
            1,
            [
                ready(creature("Raider", &[rules, &[HASTE]].concat())),
                ready(creature("Wall 1", &[])),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| forest()));
        if seat > 1 {
            scenario = scenario.battlefield(seat, [ready(creature(&format!("Wall {seat}"), &[]))]);
        }
    }
    scenario
}

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::the_owner(1, false)]
#[case::another_player(2, true)]
fn an_attack_on_its_owner_is_refused(
    #[values(2, 3, 4)] seats: usize,
    #[case] defender: usize,
    #[case] allowed: bool,
) -> Result<(), ScriptError> {
    if defender >= seats {
        return Ok(());
    }
    let restriction = r#"{"Static":{"affected":"This","mods":[{"CantAttackOwner":"Both"}]}}"#;
    let mut driver = Driver::from_scenario(table(seats, &[restriction]))?;
    driver.pass_until(Step::DeclareAttackers)?;
    assert_eq!(driver.attack(&[("Raider", defender)]).is_ok(), allowed);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::at_the_owner(1, true)]
#[case::at_another_player(2, false)]
fn only_an_attack_on_its_owner_can_be_blocked(
    #[values(2, 3, 4)] seats: usize,
    #[case] defender: usize,
    #[case] blockable: bool,
) -> Result<(), ScriptError> {
    if defender >= seats {
        return Ok(());
    }
    let restriction =
        r#"{"Static":{"affected":"This","mods":["CantBeBlockedUnlessAttackingOwner"]}}"#;
    let mut driver = Driver::from_scenario(table(seats, &[restriction]))?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Raider", defender)])?;
    driver.pass_until(Step::DeclareBlockers)?;
    let wall = format!("Wall {defender}");
    assert_eq!(
        driver.block(&[(wall.as_str(), "Raider")]).is_ok(),
        blockable
    );
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::damage_to_its_owner(1, 21)]
#[case::damage_to_another_player(2, 20)]
fn combat_damage_to_its_owner_triggers(
    #[values(2, 3, 4)] seats: usize,
    #[case] defender: usize,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    if defender >= seats {
        return Ok(());
    }
    let mut driver = Driver::from_scenario(table(seats, &[]))?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Raider", defender)])?;
    driver.pass_until(Step::DeclareBlockers)?;
    driver.block(&[])?;
    driver.pass_until(Step::End)?;
    assert_eq!(driver.state().player(PlayerId(0)).unwrap().life(), life);
    replays(&driver);
    Ok(())
}
