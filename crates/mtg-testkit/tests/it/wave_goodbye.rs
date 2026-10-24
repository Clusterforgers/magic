//! Goal: Wave Goodbye returns each creature without a +1/+1 counter on it, whoever controls it, to its owner's hand and leaves the creatures that hold one, at tables of two, three and four seats; a creature with only another kind of counter returns, a table without such a creature changes nothing, and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_cards::value::CounterKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

fn in_hand(driver: &Driver, seat: usize, name: &str) -> bool {
    names(driver.state(), Zone::Hand(pid(seat))).contains(&name.to_owned())
}

fn on_table(driver: &Driver, name: &str) -> bool {
    names(driver.state(), Zone::Battlefield).contains(&name.to_owned())
}

#[rstest]
#[case::plain_creatures_return(CounterKind::PlusOnePlusOne, 0, false)]
#[case::a_creature_with_the_counter_stays(CounterKind::PlusOnePlusOne, 2, true)]
#[case::another_kind_of_counter_does_not_save_it(CounterKind::MinusOneMinusOne, 1, false)]
fn each_creature_without_a_plus_one_counter_returns_to_its_owner(
    #[case] kind: CounterKind,
    #[case] count: u32,
    #[case] stays: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let held = |name: &str| with_counters(ready(free_creature(name, 3, 3)), kind.clone(), count);
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, ["Wave Goodbye"])
        .battlefield(
            0,
            [
                ready(island()),
                ready(island()),
                ready(island()),
                ready(island()),
                ready(free_creature("Plain Bear", 2, 2)),
                held("Marked Ogre"),
            ],
        );
    for seat in 1..seats {
        scenario =
            scenario.battlefield(seat, [ready(free_creature(&format!("Guard {seat}"), 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Wave Goodbye")?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert!(in_hand(&driver, 0, "Plain Bear"));
    for seat in 1..seats {
        assert!(
            in_hand(&driver, seat, &format!("Guard {seat}")),
            "seat {seat}"
        );
    }
    assert_eq!(in_hand(&driver, 0, "Marked Ogre"), !stays);
    assert_eq!(on_table(&driver, "Marked Ogre"), stays);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
