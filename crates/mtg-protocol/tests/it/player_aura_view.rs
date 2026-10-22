//! Goal: an Aura that enchants a player shows the player it enchants (`attached_player`) and nothing it is attached to (`attached_to`), the player shows the Auras on them (`enchanted_by`), every seat sees the same, a player who is not enchanted and a game without such an Aura leave the fields out of the JSON, and an Aura that went to the graveyard names no player.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::zones::Zone;
use mtg_protocol::{ObjectView, StateView};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, enchanting, ready};
use rstest::rstest;

fn table(seats: usize) -> Driver {
    Driver::from_scenario(Scenario::new(seats).battlefield(
        0,
        [enchanting(ready(card("Curse of Predation").unwrap()), 1)],
    ))
    .unwrap()
}

fn view(driver: &Driver, viewer: usize) -> StateView {
    StateView::for_player(driver.state(), PlayerId::from_index(viewer))
}

fn curse(view: &StateView) -> ObjectView {
    view.objects
        .iter()
        .find(|o| o.name == "Curse of Predation" && o.zone == Zone::Battlefield)
        .unwrap()
        .clone()
}

#[rstest]
fn every_seat_sees_the_player_an_aura_enchants_and_the_auras_on_a_player(
    #[values(2, 3, 4)] seats: usize,
) {
    let driver = table(seats);
    for viewer in 0..seats {
        let view = view(&driver, viewer);
        let aura = curse(&view);
        assert_eq!(aura.attached_player, Some(PlayerId::from_index(1)));
        assert_eq!(aura.attached_to, None);
        for player in &view.table.players {
            let expected = if player.seat == PlayerId::from_index(1) {
                vec![aura.id]
            } else {
                Vec::new()
            };
            assert_eq!(player.enchanted_by, expected, "seat {:?}", player.seat);
        }
    }
}

#[test]
fn the_fields_are_left_out_of_the_json_while_they_say_nothing() {
    let driver = Driver::from_scenario(Scenario::new(2)).unwrap();
    let json = serde_json::to_value(view(&driver, 0)).unwrap();
    for player in json["players"].as_array().unwrap() {
        assert!(player.get("enchanted_by").is_none());
    }
    let enchanted = serde_json::to_value(view(&table(2), 0)).unwrap();
    assert_eq!(
        enchanted["players"][1]["enchanted_by"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(enchanted["players"][0].get("enchanted_by").is_none());
    let aura = enchanted["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["name"] == "Curse of Predation")
        .unwrap();
    assert_eq!(aura["attached_player"], 1);
    assert!(aura.get("attached_to").is_none());
}
