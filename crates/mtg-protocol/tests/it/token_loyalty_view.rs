//! Goal: a token planeswalker is in every seat's view with its loyalty counters, however it was made (here a token copy made by hand), so a client can show the number on it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::manual::ManualAction;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_protocol::StateView;
use mtg_testkit::cards::walker;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

#[rstest]
fn a_token_copy_of_a_planeswalker_shows_its_loyalty_to_every_seat(
    #[values(2, 3)] seats: usize,
    #[values(0, 1)] viewer: usize,
) {
    let scenario = Scenario::new(seats).battlefield(1, [ready(walker("Jace", 2))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let jace = *driver.state().zone(Zone::Battlefield).first().unwrap();
    driver
        .manual(ManualAction::CopyToken {
            object: jace,
            count: 1,
        })
        .unwrap();
    let token = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().kind() == ObjectKind::Token)
        .unwrap();
    let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
    let shown = view.objects.iter().find(|o| o.id == token).unwrap();
    assert_eq!(shown.counters, [(CounterKind::Loyalty, 3)]);
    let json = serde_json::to_value(shown).unwrap();
    assert_eq!(json["counters"], serde_json::json!([["loyalty", 3]]));
}
