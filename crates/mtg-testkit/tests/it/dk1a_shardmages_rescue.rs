//! Goal: the compiled Shardmage's Rescue gives the creature it enchants +1/+1 for as long as it stays, and hexproof only during the turn the Aura entered, whether it was cast with flash or put on the battlefield in an earlier turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::keyword::Keyword;
use mtg_engine::characteristics::characteristics;
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

fn hexproof(driver: &Driver) -> bool {
    has_keyword(
        driver.state(),
        driver.id_of("Bear").unwrap(),
        &Keyword::Hexproof,
    )
}

fn power(driver: &Driver) -> i32 {
    characteristics(driver.state(), driver.id_of("Bear").unwrap())
        .unwrap()
        .power
        .unwrap()
}

/// Seat 0 controls the Bear and a Plains and holds the Aura; with `placed` the Aura is on the Bear already.
fn table(placed: bool) -> Result<Driver, ScriptError> {
    let filler = || (0..12).map(|_| free_creature("Filler", 1, 1));
    let mut field = vec![ready(free_creature("Bear", 2, 2)), ready(plains())];
    let mut scenario = Scenario::new(2)
        .at(2, 0, Step::Main1)
        .library(0, filler())
        .library(1, filler());
    if placed {
        field.push(attached_to(
            ready(card("Shardmage's Rescue").unwrap()),
            0,
            0,
        ));
    } else {
        scenario = scenario.hand(0, [card("Shardmage's Rescue").unwrap()]);
    }
    Driver::from_scenario(scenario.battlefield(0, field))
}

#[test]
fn an_aura_cast_this_turn_gives_hexproof_until_the_turn_ends() -> Result<(), ScriptError> {
    let mut driver = table(false)?;
    assert!(!hexproof(&driver));
    assert_eq!(power(&driver), 2);
    let bear = driver.target_named("Bear")?;
    driver.cast_at("Shardmage's Rescue", &[bear])?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 0);
    assert!(hexproof(&driver));
    assert_eq!(power(&driver), 3);
    // The next turn: the Aura is no longer new, the bonus stays.
    driver.idle_until(3, Step::Main1)?;
    assert!(!hexproof(&driver));
    assert_eq!(power(&driver), 3);
    Ok(())
}

#[rstest]
#[case::put_on_before_the_turn(true, 3)]
#[case::not_yet_cast(false, 2)]
fn an_aura_from_an_earlier_turn_gives_no_hexproof(
    #[case] placed: bool,
    #[case] bonus: i32,
) -> Result<(), ScriptError> {
    let driver = table(placed)?;
    assert!(!hexproof(&driver));
    assert_eq!(power(&driver), bonus);
    Ok(())
}
