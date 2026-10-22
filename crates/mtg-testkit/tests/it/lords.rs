//! Goal: a lord of a subtype changes the creatures of that subtype alone and whoever controls them, each lord counts once for every Sliver it reaches (itself included), a lord that says "you control" skips another seat's creatures, and a table of two to four seats gives every Sliver one bonus for each lord in play.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_testkit::cards::bear;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use mtg_engine::characteristics;

/// The power and toughness of the first permanent named `name`.
fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

/// Seat 0 controls `mine`, seat 1 `theirs`; every name is a corpus card.
fn game(mine: &[&str], theirs: &[&str]) -> Result<Driver, ScriptError> {
    let permanents = |names: &[&str]| {
        names
            .iter()
            .map(|name| ready(card(name).unwrap()))
            .collect::<Vec<_>>()
    };
    Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, permanents(mine))
            .battlefield(1, permanents(theirs)),
    )
}

#[rstest]
#[case::alone(&["Muscle Sliver"], &[], "Muscle Sliver", (2, 2))]
#[case::two_of_the_same_lord(&["Muscle Sliver", "Muscle Sliver"], &[], "Muscle Sliver", (3, 3))]
#[case::a_lord_of_all(&["Muscle Sliver", "Battle Sliver"], &[], "Muscle Sliver", (4, 2))]
#[case::a_lord_of_all_reaches_the_other_lord(&["Muscle Sliver", "Battle Sliver"], &[], "Battle Sliver", (6, 4))]
#[case::a_lord_of_yours_skips_their_sliver(&["Battle Sliver"], &["Muscle Sliver"], "Muscle Sliver", (2, 2))]
#[case::a_lord_of_yours_reaches_yours(&["Battle Sliver"], &["Muscle Sliver"], "Battle Sliver", (6, 4))]
#[case::their_lord_of_yours_skips_mine(&["Muscle Sliver"], &["Battle Sliver"], "Muscle Sliver", (2, 2))]
#[case::a_lord_of_all_reaches_theirs(&["Muscle Sliver"], &["Battle Sliver"], "Battle Sliver", (6, 4))]
fn a_lord_of_slivers_changes_the_slivers_in_its_reach(
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] probe: &str,
    #[case] expected: (i32, i32),
) -> Result<(), ScriptError> {
    let driver = game(mine, theirs)?;
    assert_eq!(stats(&driver, probe), expected);
    Ok(())
}

#[test]
fn a_creature_that_is_no_sliver_is_left_alone() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(card("Muscle Sliver").unwrap()),
            ready(card("Battle Sliver").unwrap()),
            ready(bear()),
        ],
    ))?;
    assert_eq!(stats(&driver, "Grizzly Bears"), (2, 2));
    Ok(())
}

#[rstest]
fn every_seat_of_a_table_adds_a_lord_that_reaches_all_slivers(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        scenario = scenario.battlefield(seat, [ready(card("Muscle Sliver").unwrap())]);
    }
    let driver = Driver::from_scenario(scenario)?;
    let bonus = i32::try_from(seats).unwrap();
    assert_eq!(stats(&driver, "Muscle Sliver"), (1 + bonus, 1 + bonus));
    Ok(())
}
