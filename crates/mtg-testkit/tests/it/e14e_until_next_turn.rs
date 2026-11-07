//! Goal: a delayed trigger that fires every time "until your next turn" triggers in each opponent's turn and is gone as its controller's next turn begins; one "this turn" is gone with the turn and one "for the rest of the game" stays; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::{pid, spell};

/// "Until `until`, whenever a player's upkeep begins, you gain 1 life."
fn watch(until: &str) -> std::sync::Arc<mtg_cards::CardDefinition> {
    let lasts = match until {
        "this_turn" => r#""this_turn":true"#.to_owned(),
        other => format!(r#""until":"{other}""#),
    };
    spell(
        "Watch",
        CardType::Sorcery,
        "[]",
        &format!(
            r#"[{{"DelayedTrigger":{{"when":{{"StepBegins":{{"step":"Upkeep"}}}},"once":false,{lasts},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}]"#
        ),
    )
}

#[rstest]
#[case::until_your_next_turn("UntilYourNextTurn", |seats: i32| seats - 1)]
#[case::this_turn("this_turn", |_| 0)]
#[case::forever("Forever", |seats: i32| seats)]
fn the_trigger_lasts_until_the_next_turn_of_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[case] until: &str,
    #[case] gained: fn(i32) -> i32,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).hand(0, vec![watch(until)]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let before = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Watch")?;
    driver.pass()?;
    let turns = u32::try_from(seats).unwrap();
    // Every opponent's upkeep, then this player's next upkeep (turn seats + 1).
    driver.idle_until(turns + 1, Step::Draw)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life - before, gained(i32::try_from(seats).unwrap()));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
