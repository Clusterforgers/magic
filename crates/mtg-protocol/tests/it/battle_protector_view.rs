//! Goal: a battle shows the seat that protects it (`protector`) to every seat, and a game without a battle leaves the field out of the JSON.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::zones::Zone;
use mtg_protocol::{ObjectView, StateView};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, protected_by, ready};
use rstest::rstest;

const BATTLE: &str = "Invasion of Theros // Ephara, Ever-Sheltering";

fn view(driver: &Driver, viewer: usize) -> StateView {
    StateView::for_player(driver.state(), PlayerId::from_index(viewer))
}

fn on_battlefield(view: &StateView, name: &str) -> ObjectView {
    view.objects
        .iter()
        .find(|o| o.name.starts_with(name) && o.zone == Zone::Battlefield)
        .unwrap()
        .clone()
}

#[rstest]
fn every_seat_sees_who_protects_a_battle(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] protector: usize,
) {
    let protector = protector.min(seats - 1);
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [protected_by(ready(card(BATTLE).unwrap()), protector)]),
    )
    .unwrap();
    for viewer in 0..seats {
        let battle = on_battlefield(&view(&driver, viewer), "Invasion of Theros");
        assert_eq!(battle.protector, Some(PlayerId::from_index(protector)));
    }
}

#[test]
fn the_field_is_left_out_of_the_json_while_there_is_no_protector() {
    let driver =
        Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(card("Forest").unwrap())]))
            .unwrap();
    let json = serde_json::to_value(view(&driver, 0)).unwrap();
    assert!(
        json["objects"]
            .as_array()
            .unwrap()
            .iter()
            .all(|o| o.get("protector").is_none())
    );
}
