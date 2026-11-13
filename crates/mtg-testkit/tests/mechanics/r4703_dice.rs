//! Goal: several dice read together ("Roll three six-sided dice. For each different result, create a 1/1 white Clown
//! Robot artifact creature token.", Circuits Act) roll every die from the game's random stream, log each roll, and
//! create one token for each different number rolled, over many seeds at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use crate::common::names;
use mtg_engine::events::Event;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

#[rstest]
fn one_token_for_each_different_result(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 4, 5, 6, 7, 8)] seed: u64,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .seed(seed)
        .battlefield(0, lands(3, mountain))
        .hand(0, vec![card("Circuits Act").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let mut events = driver.cast("Circuits Act")?;
    for _ in 0..40 {
        if driver.state().zone(Zone::Stack).is_empty() {
            break;
        }
        events.extend(driver.answer_passively()?);
    }
    let mut rolled: Vec<u32> = events
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled { sides: 6, result, .. } => Some(*result),
            _ => None,
        })
        .collect();
    assert_eq!(rolled.len(), 3, "three dice are rolled and logged");
    assert!(rolled.iter().all(|result| (1..=6).contains(result)));
    rolled.sort_unstable();
    rolled.dedup();
    let robots = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| name.as_str() == "Clown Robot")
        .count();
    assert_eq!(robots, rolled.len());
    Ok(())
}
