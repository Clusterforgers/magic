//! Goal: a permanent that is a copy is shown to every seat as the object it copies (its name, types and abilities), with a marker that names the original while the viewer can see that very object and only says it is a copy once the original has left; an object that is not a copy has no marker, no hidden information is added at tables of two, three and four seats, and the wire form of an object that is no copy is unchanged.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::events::{Event, ZoneCause};
use mtg_engine::fold::apply_event;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, characteristics};
use mtg_protocol::{CopyView, ObjectView, StateView};
use mtg_testkit::cards::{island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const HERO: &str = "Hero of Bladehold";

fn view_of(state: &GameState, viewer: usize, id: ObjectId) -> ObjectView {
    StateView::for_player(state, PlayerId::from_index(viewer))
        .objects
        .into_iter()
        .find(|o| o.id == id)
        .unwrap()
}

/// A game in which seat 0 has cast Cackling Counterpart on its Hero, so that a Hero token copies it.
fn with_a_token_copy(seats: usize) -> (Driver, ObjectId, ObjectId) {
    let mut lands: Vec<_> = (0..3).map(|_| ready(island())).collect();
    lands.push(ready(plains()));
    lands.push(ready(card(HERO).unwrap()));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [card("Cackling Counterpart").unwrap()])
            .battlefield(0, lands),
    )
    .unwrap();
    let hero = driver.id_of(HERO).unwrap();
    let target = driver.target_named(HERO).unwrap();
    driver.begin_cast("Cackling Counterpart").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver
        .pay_tapping(&["Island", "Island", "Island"], 0)
        .unwrap();
    while !(driver.state().stack().is_empty() && driver.state().resolving().is_none()) {
        driver.answer_passively().unwrap();
    }
    let token = *driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| **id != hero && driver.state().object(**id).unwrap().card().name == HERO)
        .unwrap();
    (driver, hero, token)
}

#[rstest]
fn a_copy_is_shown_as_what_it_copies_with_a_marker_naming_the_original(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] viewer: usize,
) {
    let (driver, hero, token) = with_a_token_copy(seats);
    let shown = view_of(driver.state(), viewer, token);
    let original = view_of(driver.state(), viewer, hero);
    assert_eq!(shown.name, HERO);
    assert_eq!(shown.copy, Some(CopyView { of: Some(hero) }));
    assert_eq!((shown.power, shown.toughness), (Some(3), Some(4)));
    assert_eq!(shown.types, original.types);
    assert_eq!(shown.abilities, original.abilities);
    assert_eq!(shown.keywords, original.keywords);
    assert_eq!(original.copy, None);
    assert_eq!(
        characteristics(driver.state(), token).unwrap().power,
        shown.power
    );
}

#[rstest]
fn the_marker_only_says_it_is_a_copy_once_the_original_has_left(#[values(2, 3, 4)] seats: usize) {
    let (driver, hero, token) = with_a_token_copy(seats);
    let mut state = driver.state().clone();
    let died = Event::ZoneChanged {
        object: hero,
        from: Zone::Battlefield,
        to: Zone::Graveyard(PlayerId(0)),
        cause: ZoneCause::Destroyed,
    };
    apply_event(&mut state, &died).unwrap();
    for viewer in 0..seats {
        let shown = view_of(&state, viewer, token);
        assert_eq!(shown.name, HERO);
        assert_eq!(shown.copy, Some(CopyView { of: None }), "seat {viewer}");
    }
}

#[test]
fn the_wire_form_of_an_object_that_is_no_copy_has_no_copy_field() {
    let (driver, hero, token) = with_a_token_copy(2);
    let original = serde_json::to_value(view_of(driver.state(), 1, hero)).unwrap();
    assert!(original.get("copy").is_none());
    let copy = serde_json::to_value(view_of(driver.state(), 1, token)).unwrap();
    assert_eq!(copy["copy"], serde_json::json!({ "of": hero.get() }));
}
