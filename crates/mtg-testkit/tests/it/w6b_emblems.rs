//! Goal: emblems work at tables of two, three and four seats: an emblem lands in the command zone of each player the
//! effect names and stays after its source is gone; its static ability changes only its owner's creatures, its
//! triggered ability triggers from the command zone with "you" as its owner, it leaves the game with its owner, and the
//! log of such a game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::object::ObjectKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, instant, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Creatures you control get +1/+1.
const ANTHEM: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;
/// Whenever a player draws a card, you lose 2 life.
const DRAIN: &str = r#"{"Triggered":{"when":{"Drew":{}},"effects":[{"LoseLife":{"amount":2}}]}}"#;

/// An instant for `{U}`: "`who` gets an emblem with `ability`".
fn gift(name: &str, who: &str, ability: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Spell":{{"effects":[{{"CreateEmblem":{{"who":"{who}","abilities":[{ability}]}}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(Arc::unwrap_or_clone(instant(name, 0, &[Color::Blue])).with_rules(rules))
}

/// Seat 0 holds the gift; every seat has a 1/1 Bear and an Island.
fn table(seats: usize, gift_card: Arc<CardDefinition>) -> Scenario {
    let mut scenario = Scenario::new(seats).hand(0, [gift_card]);
    for seat in 0..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(island()),
                ready(free_creature(&format!("Bear {seat}"), 1, 1)),
            ],
        );
    }
    scenario
}

fn cast_and_resolve(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.idle_until(1, Step::Main1)?;
    driver.cast(name)?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

fn emblems(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Command(pid(seat)))
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().kind() == ObjectKind::Emblem)
        .count()
}

fn power(driver: &Driver, name: &str) -> i32 {
    let id = driver.id_of(name).unwrap();
    characteristics(driver.state(), id).unwrap().power.unwrap()
}

#[rstest]
#[case::you("You", &[0])]
#[case::each_opponent("Opponents", &[1, 2, 3])]
#[case::each_player("EachPlayer", &[0, 1, 2, 3])]
fn w6b_an_anthem_emblem_pumps_only_its_owners_creatures(
    #[case] who: &str,
    #[case] owners: &[usize],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, gift("Gift", who, ANTHEM)))?;
    cast_and_resolve(&mut driver, "Gift")?;
    for seat in 0..seats {
        let owns = owners.contains(&seat);
        assert_eq!(emblems(&driver, seat), usize::from(owns), "seat {seat}");
        assert_eq!(power(&driver, &format!("Bear {seat}")), 1 + i32::from(owns));
    }
    // The gift is in the graveyard; the emblem still works next turn.
    driver.idle_until(2, Step::Main1)?;
    let owns = owners.contains(&0);
    assert_eq!(power(&driver, "Bear 0"), 1 + i32::from(owns));
    Ok(())
}

#[rstest]
fn w6b_a_triggered_emblem_makes_its_owner_lose_life_for_each_draw(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, gift("Curse", "Opponents", DRAIN)))?;
    cast_and_resolve(&mut driver, "Curse")?;
    // Seat 1 draws for its turn: each opponent of seat 0 loses 2 life, seat 0 none.
    driver.idle_until(2, Step::Main1)?;
    let life = |seat: usize| driver.state().player(pid(seat)).unwrap().life();
    assert_eq!(life(0), 20);
    for seat in 1..seats {
        assert_eq!(life(seat), 18, "seat {seat}");
    }
    Ok(())
}

#[rstest]
fn w6b_an_emblem_leaves_the_game_with_its_owner(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, gift("Gift", "EachPlayer", ANTHEM)))?;
    cast_and_resolve(&mut driver, "Gift")?;
    let emblem = driver.state().zone(Zone::Command(pid(2)))[0];
    driver.concede(pid(2))?;
    driver.pass()?;
    assert!(driver.state().object(emblem).is_none());
    assert_eq!(emblems(&driver, 0), 1);
    Ok(())
}

#[rstest]
fn w6b_the_log_of_a_game_with_emblems_replays(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, gift("Curse", "EachPlayer", DRAIN));
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    cast_and_resolve(&mut driver, "Curse")?;
    driver.idle_until(2, Step::Main1)?;
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
