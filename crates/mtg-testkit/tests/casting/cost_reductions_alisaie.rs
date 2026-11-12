//! Goal: "The second spell you cast each turn costs {2} less to cast" (Alisaie Leveilleur) takes {2} off the second spell of the caster's turn only: not the first, not the third, not a spell another player cast in between, and the count starts again with the next turn, at every table size and with the log replaying to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use crate::cost_reductions_more::{copies, cost, resolve, table, until};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// The scenario: Alisaie in play for seat 0, four Renewed Faiths in its hand and, on seat 1, a Lightning Bolt with the
/// mana to cast it.
fn scenario(seats: usize) -> Scenario {
    let hand = ["Renewed Faith"; 4];
    table(seats, &hand, copies("Alisaie Leveilleur", 1), vec![])
        .hand_named(1, ["Lightning Bolt"])
        .battlefield(1, lands(2, mountain))
}

/// The cost of casting Renewed Faith now, paid by tapping `lands`.
fn faith(driver: &mut Driver, lands: &[&str]) -> Result<String, ScriptError> {
    driver.begin_cast("Renewed Faith")?;
    let paid = driver.pay_options()?.cost.to_string();
    driver.pay_tapping(lands, 0)?;
    Ok(paid)
}

/// The state of the log replayed equals the live state.
fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
fn the_second_spell_costs_less_and_the_first_and_third_do_not(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = scenario(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(scenario(seats))?;
    let mut costs = Vec::new();
    for lands in [
        &["Plains", "Island", "Island"][..],
        &["Plains"][..],
        &["Plains", "Swamp", "Swamp"][..],
    ] {
        costs.push(faith(&mut driver, lands)?);
        resolve(&mut driver, seats)?;
    }
    assert_eq!(costs, [cost("{2}{W}"), cost("{W}"), cost("{2}{W}")]);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_spell_of_another_player_is_not_one_of_the_casters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = scenario(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(scenario(seats))?;
    assert_eq!(
        faith(&mut driver, &["Plains", "Island", "Island"])?,
        cost("{2}{W}")
    );
    // An opponent answers with a spell of its own while Renewed Faith is on the stack.
    driver.pass()?;
    driver.begin_cast("Lightning Bolt")?;
    let target = driver.target_player(0);
    driver.choose_targets(&[target])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    until(&mut driver, 0)?;
    // It is the second spell of seat 0, and the third of the turn.
    assert_eq!(faith(&mut driver, &["Plains"])?, cost("{W}"));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_count_starts_again_with_the_next_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = scenario(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(scenario(seats))?;
    for lands in [&["Plains", "Island", "Island"][..], &["Plains"][..]] {
        faith(&mut driver, lands)?;
        resolve(&mut driver, seats)?;
    }
    // The next turn of seat 0 is `seats` turns later.
    driver.idle_until(2 + u32::try_from(seats).unwrap(), Step::Main1)?;
    let mut costs = Vec::new();
    for lands in [&["Plains", "Island", "Island"][..], &["Plains"][..]] {
        costs.push(faith(&mut driver, lands)?);
        resolve(&mut driver, seats)?;
    }
    assert_eq!(costs, [cost("{2}{W}"), cost("{W}")]);
    replays(&driver, &setup);
    Ok(())
}
