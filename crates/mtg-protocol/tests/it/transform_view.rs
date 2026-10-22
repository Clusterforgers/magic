//! Goal: the views of a transformed permanent, a fused split spell and the day or night show every seat the same, at two to four seats: a permanent turned over is the other face's name, power, toughness and keywords with the back face as its face, a fused spell is the card as a whole with the cost of both halves, the turn view says day or night once either has begun and leaves it out before, a card in a hand stays hidden from the seats that do not own it, the turned-over and day-and-night events reach every seat whole, and a table view with the day or night round-trips through its JSON.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::day_night::DayNight;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, apply_event};
use mtg_protocol::{EventView, ObjectView, TableView, project_events};
use mtg_testkit::cards::{forest, mountain, plains};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use mtg_engine::choose::OptionChoice;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

fn table(seats: usize, hand: &[&str], battlefield: &[&str]) -> Driver {
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 6])
        .hand_named(0, hand.iter().copied())
        .battlefield(
            0,
            (0..3)
                .map(|_| ready(mountain()))
                .chain((0..2).map(|_| ready(plains()))),
        );
    for other in 1..seats {
        scenario = scenario.library(other, vec![forest(); 6]);
    }
    scenario = scenario.battlefield_named(seats - 1, battlefield.iter().copied());
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
fn a_transformed_permanent_is_the_other_face_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let driver = table(
        seats,
        &[],
        &["Ludevic's Test Subject // Ludevic's Abomination"],
    );
    let id = driver.state().zone(Zone::Battlefield)[5];
    let mut turned = driver.state().clone();
    for viewer in 0..seats {
        let view = ObjectView::of(&turned, seat(viewer), id).unwrap();
        assert_eq!(
            (view.name.as_str(), view.face, view.power, view.toughness),
            ("Ludevic's Test Subject", None, Some(0), Some(3))
        );
        assert!(view.keywords.contains(&"Defender".to_owned()));
    }
    apply_event(
        &mut turned,
        &Event::Transformed {
            object: id,
            face: Some(1),
        },
    )
    .unwrap();
    for viewer in 0..seats {
        let view = ObjectView::of(&turned, seat(viewer), id).unwrap();
        assert_eq!(
            (view.name.as_str(), view.face, view.power, view.toughness),
            ("Ludevic's Abomination", Some(1), Some(13), Some(13))
        );
        assert!(view.keywords.contains(&"Trample".to_owned()));
        assert!(!view.keywords.contains(&"Defender".to_owned()));
    }
}

#[rstest]
fn a_fused_spell_is_the_card_as_a_whole_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats, &["Wear // Tear"], &["Sol Ring", "Glorious Anthem"]);
    driver.begin_cast("Wear // Tear").unwrap();
    // The faces are asked in order: Wear, Tear, both.
    driver
        .answer(Answer::ChooseOption(OptionChoice(2)))
        .unwrap();
    let artifact = driver.target_named("Sol Ring").unwrap();
    driver.choose_targets(&[artifact]).unwrap();
    let enchantment = driver.target_named("Glorious Anthem").unwrap();
    driver.choose_targets(&[enchantment]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let spell = driver.state().zone(Zone::Stack)[0];
    for viewer in 0..seats {
        let view = ObjectView::of(driver.state(), seat(viewer), spell).unwrap();
        assert_eq!((view.name.as_str(), view.face), ("Wear // Tear", Some(2)));
        assert_eq!(view.mana_cost.unwrap().to_string(), "{1}{W}{R}");
    }
}

#[rstest]
fn a_card_in_a_hand_stays_hidden_from_the_other_seats(#[values(2, 3, 4)] seats: usize) {
    let driver = table(seats, &["Wear // Tear"], &[]);
    let id = driver.state().zone(Zone::Hand(seat(0)))[0];
    assert!(ObjectView::of(driver.state(), seat(0), id).is_some());
    for other in 1..seats {
        assert!(ObjectView::of(driver.state(), seat(other), id).is_none());
    }
}

#[rstest]
fn the_turn_view_says_day_or_night_once_it_has_begun(#[values(2, 3, 4)] seats: usize) {
    let driver = table(seats, &[], &["Tavern Ruffian // Tavern Smasher"]);
    let mut state = driver.state().clone();
    for viewer in 0..seats {
        let view = TableView::of(&state, seat(viewer));
        assert_eq!(view.turn.day_night, Some(DayNight::Day));
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["turn"]["day_night"], "Day");
        let back: TableView = serde_json::from_value(json).unwrap();
        assert_eq!(back, view);
    }
    apply_event(
        &mut state,
        &Event::DayNightChanged {
            to: DayNight::Night,
        },
    )
    .unwrap();
    for viewer in 0..seats {
        assert_eq!(
            TableView::of(&state, seat(viewer)).turn.day_night,
            Some(DayNight::Night)
        );
    }
    let plain = table(seats, &[], &[]);
    let view = TableView::of(plain.state(), seat(0));
    assert_eq!(view.turn.day_night, None);
    assert!(
        serde_json::to_value(&view).unwrap()["turn"]
            .get("day_night")
            .is_none()
    );
}

#[rstest]
fn the_events_of_a_transformation_and_of_the_night_reach_every_seat_whole(
    #[values(2, 3, 4)] seats: usize,
) {
    let driver = table(seats, &[], &["Delver of Secrets // Insectile Aberration"]);
    let object = driver.state().zone(Zone::Battlefield)[5];
    let events = [
        Event::Transformed {
            object,
            face: Some(1),
        },
        Event::DayNightChanged {
            to: DayNight::Night,
        },
    ];
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(driver.state(), &events, &viewers, 1).unwrap();
    for batch in batches {
        let shown: Vec<EventView> = events.iter().cloned().map(EventView::Public).collect();
        assert_eq!(batch.events, shown);
    }
}
