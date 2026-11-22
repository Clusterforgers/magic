//! Goal: a cast permission that names the card its resolution bound in `what` ("You may cast that card for as long as it remains exiled", `Pred::Is(Ref::Bound)`) covers that card after the resolution ends, at tables of two to four seats: `Ramirez DePietro, Pillager` deals combat damage, exiles the top card of the damaged player's library, and its controller may cast that card from exile after the trigger has resolved; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn table(seats: usize) -> Scenario {
    let mut mine = vec![ready(card("Ramirez DePietro, Pillager").unwrap())];
    mine.extend((0..2).map(|_| ready(forest())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .library(1, [bear(), bear()]);
    for seat in 2..seats {
        scenario = scenario.library(seat, [bear()]);
    }
    scenario
}

/// The cards in exile the deciding player may cast now.
fn castable_in_exile(driver: &Driver) -> Vec<mtg_core::ids::ObjectId> {
    match &driver.engine().decision().unwrap().kind {
        DecisionKind::Priority(options) => options
            .castable
            .iter()
            .copied()
            .filter(|id| {
                driver
                    .state()
                    .object(*id)
                    .map(mtg_engine::object::GameObject::zone)
                    == Some(Zone::Exile)
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[rstest]
fn the_exiled_card_stays_castable(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = table(seats);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let earlier: Vec<_> = driver.state().zone(Zone::Exile).to_vec();
    assert_eq!(castable_in_exile(&driver), Vec::new());
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ramirez DePietro, Pillager", 1)])?;
    driver.pass_until(Step::Main2)?;
    let exiled: Vec<_> = driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .copied()
        .filter(|id| !earlier.contains(id))
        .collect();
    assert_eq!(
        exiled.len(),
        1,
        "the top card of the damaged player's library"
    );
    assert_eq!(
        castable_in_exile(&driver),
        exiled,
        "that card alone may be cast"
    );
    let name = driver
        .state()
        .object(exiled[0])
        .unwrap()
        .card()
        .name
        .clone();
    driver.cast(&name)?;
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
