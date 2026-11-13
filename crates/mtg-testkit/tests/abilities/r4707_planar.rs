//! Goal: in a game without a planar deck, Fractured Powerstone's "{T}: Roll the planar die. Activate only as a sorcery."
//! is offered only at sorcery speed and resolves with no effect (its ruling: "In non-Planechase games, Fractured
//! Powerstone's second ability will have no effect."), at two to four seats, with Ichor Elixir and Susan Foreman on the
//! battlefield changing nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::turn::Step;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

#[rstest]
fn r4707_rolling_the_planar_die_does_nothing(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] replacements: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Fractured Powerstone").unwrap())];
    if replacements {
        mine.push(ready(card("Ichor Elixir").unwrap()));
        mine.push(ready(card("Susan Foreman").unwrap()));
    }
    let scenario = Scenario::new(seats).battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario)?;
    let offered = driver.offered_abilities("Fractured Powerstone")?;
    assert_eq!(
        offered.len(),
        1,
        "the roll is the one ability offered at priority"
    );
    let roll = offered[0];
    let before: Vec<(usize, i32)> = (0..seats)
        .map(|seat| {
            (
                driver
                    .state()
                    .zone(mtg_engine::zones::Zone::Hand(pid(seat)))
                    .len(),
                driver.state().player(pid(seat)).unwrap().life(),
            )
        })
        .collect();
    driver.begin_ability("Fractured Powerstone", roll)?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(driver.state().stack(), []);
    let powerstone = driver.id_of("Fractured Powerstone")?;
    assert!(
        driver
            .state()
            .object(powerstone)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    for (seat, (hand, life)) in before.into_iter().enumerate() {
        assert_eq!(
            driver
                .state()
                .zone(mtg_engine::zones::Zone::Hand(pid(seat)))
                .len(),
            hand
        );
        assert_eq!(driver.state().player(pid(seat)).unwrap().life(), life);
    }
    assert_eq!(driver.state().turn().step(), Step::Main1);
    Ok(())
}

#[rstest]
fn r4707_the_planar_die_waits_for_sorcery_speed(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("Fractured Powerstone").unwrap())])
        .at(1, 0, Step::Upkeep);
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(
        driver.offered_abilities("Fractured Powerstone")?,
        Vec::<usize>::new()
    );
    Ok(())
}
