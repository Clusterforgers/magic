//! Goal: which doors of a Room are unlocked is a public fact of its permanent for every seat (both locked as the Room enters unplayed, one unlocked after the special action, none for any other permanent), and the view names the Room by the door unlocked.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_protocol::StateView;
use mtg_testkit::cards::{forest, plains};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const ROOM: &str = "Grand Entryway // Elegant Rotunda";

fn scenario(seats: usize) -> Scenario {
    let room = mtg_testkit::corpus::card(ROOM).unwrap();
    let mut lands: Vec<_> = (0..3).map(|_| ready(plains())).collect();
    lands.push(ready(room));
    let mut scenario = Scenario::new(seats).battlefield(0, lands);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 12]);
    }
    scenario
}

fn room_in(driver: &Driver) -> ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().dk1c5_doors(*id).is_some())
        .unwrap()
}

#[rstest]
fn the_doors_of_a_room_are_public_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let mut driver = Driver::from_scenario(scenario(seats)).unwrap();
    let id = room_in(&driver);
    let doors = |driver: &Driver, viewer: usize, id: ObjectId| {
        let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
        let shown = view.objects.iter().find(|o| o.id == id).unwrap();
        (shown.permanent.as_ref().unwrap().doors, shown.name.clone())
    };
    for viewer in 0..seats {
        assert_eq!(
            doors(&driver, viewer, id),
            (Some([false, false]), String::new())
        );
    }
    driver
        .answer(Answer::Priority(PriorityAction::UnlockDoor(id, 0)))
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    for viewer in 0..seats {
        assert_eq!(
            doors(&driver, viewer, id),
            (Some([true, false]), "Grand Entryway".to_owned())
        );
    }
    let land = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().dk1c5_doors(*id).is_none())
        .unwrap();
    assert_eq!(doors(&driver, 0, land).0, None, "a land has no doors");
}
