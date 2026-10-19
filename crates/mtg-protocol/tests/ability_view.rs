//! Goal: an ability on the stack reaches every seat as an object of its own, named after its source with nothing else of the source's printed side, with its source and targets; a spell on the stack shows its targets; a card shows none of this, so the JSON of a vanilla game is what it was; and the ability comes and goes through the batches like any object.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::choose::Target;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_protocol::{EventView, ObjectView, StateView, project_events};
use mtg_testkit::cards::{burn, mountain, pinger};
use mtg_testkit::{Driver, Scenario, ready};

const SEATS: [PlayerId; 2] = [PlayerId(0), PlayerId(1)];

fn view_of(driver: &Driver, viewer: PlayerId, id: mtg_core::ids::ObjectId) -> ObjectView {
    StateView::for_player(driver.state(), viewer)
        .objects
        .into_iter()
        .find(|o| o.id == id)
        .unwrap()
}

#[test]
fn an_ability_on_the_stack_is_an_object_named_after_its_source_with_its_targets() {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(pinger("Pinger", 1)), ready(mountain())]),
    )
    .unwrap();
    let pinger_id = driver.id_of("Pinger").unwrap();
    driver.begin_activation("Pinger").unwrap();
    let at = driver.target_player(1);
    driver.choose_targets(&[at]).unwrap();
    let ability = *driver.state().stack().last().unwrap();
    for viewer in SEATS {
        let shown = view_of(&driver, viewer, ability);
        assert_eq!(shown.kind, ObjectKind::Ability);
        assert_eq!(shown.source, Some(pinger_id));
        assert_eq!(shown.name, "Pinger");
        assert_eq!(shown.zone, Zone::Stack);
        assert_eq!(shown.controller, PlayerId(0));
        assert_eq!(shown.targets, [vec![at]]);
        // Nothing of the printed side of the source: an ability has no cost, types, colours or stats.
        assert!(shown.mana_cost.is_none() && shown.types.is_empty() && shown.supertypes.is_empty());
        assert!(shown.subtypes.is_empty() && shown.colors.is_empty() && shown.abilities.is_empty());
        assert!(shown.power.is_none() && shown.toughness.is_none() && shown.permanent.is_none());
        let table = StateView::for_player(driver.state(), viewer).table;
        assert_eq!(table.stack.cards, Some(vec![ability]));
        // The source is still the card it was.
        let source = view_of(&driver, viewer, pinger_id);
        assert_eq!(source.kind, ObjectKind::Card);
        assert!(source.source.is_none() && source.targets.is_empty());
        assert!(source.mana_cost.is_some());
    }
}

#[test]
fn a_spell_on_the_stack_shows_what_it_targets() {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain())]),
    )
    .unwrap();
    let at = driver.target_player(1);
    driver.cast_at("Bolt", &[at]).unwrap();
    let spell = *driver.state().stack().last().unwrap();
    let shown = view_of(&driver, PlayerId(1), spell);
    assert_eq!((shown.kind, shown.source), (ObjectKind::Card, None));
    assert_eq!(shown.targets, [vec![at]]);
    assert_eq!(shown.name, "Bolt");
    // When it has resolved it is a card in a graveyard and keeps nothing it announced.
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(
        view_of(&driver, PlayerId(1), spell).targets,
        Vec::<Vec<Target>>::new()
    );
}

#[test]
fn the_json_of_a_card_has_none_of_the_fields_of_an_ability() {
    let driver =
        Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(mountain())])).unwrap();
    let land = driver.id_of("Mountain").unwrap();
    let json = serde_json::to_value(view_of(&driver, PlayerId(0), land)).unwrap();
    let keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    for absent in ["kind", "source", "targets"] {
        assert!(!keys.contains(&absent), "{absent} in {keys:?}");
    }
}

#[test]
fn the_ability_arrives_in_a_batch_and_is_told_to_be_gone_when_it_has_resolved() {
    let mut driver =
        Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(pinger("Pinger", 1))]))
            .unwrap();
    driver.begin_activation("Pinger").unwrap();
    let before = driver.state().clone();
    let at = driver.target_player(1);
    let first = driver.choose_targets(&[at]).unwrap();
    let ability = *driver.state().stack().last().unwrap();
    let batches = project_events(&before, &first, &SEATS, before_len(&driver, &first)).unwrap();
    for batch in &batches {
        assert!(
            batch
                .objects
                .iter()
                .any(|o| o.id == ability && o.kind == ObjectKind::Ability)
        );
        assert!(
            batch
                .events
                .iter()
                .any(|e| matches!(e, EventView::Public(Event::AbilityPutOnStack(_))))
        );
    }
    let before = driver.state().clone();
    let mut events = Vec::new();
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass().unwrap());
    }
    let batches = project_events(&before, &events, &SEATS, 0).unwrap();
    for batch in &batches {
        assert!(batch.gone.contains(&ability), "{:?}", batch.gone);
        assert_eq!(batch.table.stack.count, 0);
    }
}

/// The log position at which `events` began.
fn before_len(driver: &Driver, events: &[Event]) -> u64 {
    (driver.engine().events().len() - events.len()) as u64
}

#[test]
fn a_view_with_an_ability_survives_the_trip_through_json() {
    let mut driver =
        Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(pinger("Pinger", 1))]))
            .unwrap();
    driver.begin_activation("Pinger").unwrap();
    let at = driver.target_player(1);
    driver.choose_targets(&[at]).unwrap();
    let view = StateView::for_player(driver.state(), PlayerId(1));
    let text = serde_json::to_string(&view).unwrap();
    assert_eq!(serde_json::from_str::<StateView>(&text).unwrap(), view);
    assert!(text.contains(r#""kind":"Ability""#));
}
