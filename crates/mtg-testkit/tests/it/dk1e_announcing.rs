//! Goal: a manual action taken out of band while a cast is being announced leaves what state-based actions would do until the announcement ends (CR 704.3), so the invariants hold at the question under way and the creature it left at zero toughness dies as soon as the cast is backed out of, at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_engine::manual::ManualAction;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

#[rstest]
fn a_creature_a_manual_action_leaves_dying_waits_for_the_end_of_the_announcement(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] by_damage: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain()), ready(free_creature("Bear", 2, 2))])
            .library(0, (0..10).map(|_| free_creature("Filler", 1, 1))),
    )?;
    let bear = driver.id_of("Bear")?;
    driver.begin_cast("Bolt")?;
    let action = if by_damage {
        ManualAction::Damage {
            source: bear,
            target: mtg_engine::combat::DamageTarget::Object(bear),
            amount: 2,
        }
    } else {
        ManualAction::Pump {
            object: bear,
            power: -2,
            toughness: -2,
        }
    };
    driver.manual_by(pid(0), &action)?;
    assert_eq!(
        names(driver.state(), Zone::Battlefield),
        ["Mountain", "Bear"]
    );
    driver.cancel_targets()?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Bear"]);
    Ok(())
}
