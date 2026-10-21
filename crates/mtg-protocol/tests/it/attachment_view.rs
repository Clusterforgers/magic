//! Goal: an object in a view carries the counters on a permanent and what it is attached to and what is attached to it, to every seat alike, never for a card off the battlefield and never naming an object the viewer cannot see; a host that left, or came back as a new object, holds nothing; and the fields are left out of the JSON while they say nothing so a game without them keeps its wire form.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, apply_event};
use mtg_protocol::{ObjectView, StateView};
use mtg_testkit::cards::{bear, free_creature, walker};
use mtg_testkit::{Driver, Scenario, attached_to, ready, with_counters};
use rstest::rstest;
use serde_json::json;

/// An Aura that enchants a creature.
fn aura(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_subtype("Aura")
            .with_rules(Ability::Keyword(Keyword::Enchant(Selector(vec![
                Pred::Type(CardType::Creature),
            ])))),
    )
}

fn equipment(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_subtype("Equipment"),
    )
}

/// Seat 0 has a Bear with two +1/+1 counters and a charge counter, a Blade attached to it, a Jace with three loyalty
/// counters and a Halo attached to seat 1's Ogre; seat 1 has the Ogre; every seat has a card in hand.
fn table(seats: usize) -> Driver {
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                with_counters(
                    with_counters(ready(bear()), CounterKind::Charge, 1),
                    CounterKind::PlusOnePlusOne,
                    2,
                ),
                attached_to(ready(equipment("Blade")), 0, 0),
                with_counters(ready(walker("Jace", 3)), CounterKind::Loyalty, 3),
                attached_to(ready(aura("Halo")), 1, 0),
            ],
        )
        .battlefield(1, [ready(free_creature("Ogre", 4, 4))]);
    for seat in 0..seats {
        scenario = scenario.hand(seat, [bear()]);
    }
    Driver::from_scenario(scenario).unwrap()
}

fn view(driver: &Driver, viewer: usize) -> StateView {
    StateView::for_player(driver.state(), PlayerId::from_index(viewer))
}

fn named(view: &StateView, name: &str) -> ObjectView {
    view.objects
        .iter()
        .find(|o| o.name == name && o.zone == Zone::Battlefield)
        .unwrap()
        .clone()
}

#[rstest]
fn every_seat_sees_the_counters_and_attachments_of_a_permanent(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] viewer: usize,
) {
    let driver = table(seats);
    let view = view(&driver, viewer);
    let id = |name: &str| driver.id_of(name).unwrap();
    let bear = named(&view, "Grizzly Bears");
    assert_eq!(
        bear.counters,
        [(CounterKind::PlusOnePlusOne, 2), (CounterKind::Charge, 1)]
    );
    assert_eq!(bear.attachments, [id("Blade")]);
    assert_eq!(bear.attached_to, None);
    let blade = named(&view, "Blade");
    assert_eq!(blade.attached_to, Some(id("Grizzly Bears")));
    assert_eq!(blade.counters, []);
    assert_eq!(named(&view, "Jace").counters, [(CounterKind::Loyalty, 3)]);
    // An Aura of one seat on a creature of another.
    assert_eq!(named(&view, "Halo").attached_to, Some(id("Ogre")));
    assert_eq!(named(&view, "Ogre").attachments, [id("Halo")]);
}

#[rstest]
fn nothing_off_the_battlefield_holds_counters_or_attachments_and_every_id_named_is_in_the_view(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] viewer: usize,
) {
    let driver = table(seats);
    let view = view(&driver, viewer);
    let seen: Vec<ObjectId> = view.objects.iter().map(|o| o.id).collect();
    for object in &view.objects {
        if object.zone != Zone::Battlefield {
            assert!(
                object.counters.is_empty()
                    && object.attachments.is_empty()
                    && object.attached_to.is_none(),
                "{} in {:?}",
                object.name,
                object.zone
            );
        }
        for named in object.attachments.iter().chain(object.attached_to.iter()) {
            assert!(seen.contains(named), "{} names {named:?}", object.name);
        }
    }
    // The fields say the same to every seat: only the hands differ.
    let other = StateView::for_player(driver.state(), PlayerId::from_index(seats - 1));
    let on_table = |view: &StateView| -> Vec<ObjectView> {
        view.objects
            .iter()
            .filter(|o| o.zone == Zone::Battlefield)
            .cloned()
            .collect()
    };
    assert_eq!(on_table(&view), on_table(&other));
}

#[test]
fn the_fields_are_left_out_of_the_json_while_they_say_nothing() {
    let driver = table(2);
    let view = view(&driver, 0);
    let ogre = serde_json::to_value(named(&view, "Ogre")).unwrap();
    let halo_id = driver.id_of("Halo").unwrap().get();
    assert!(ogre.get("counters").is_none() && ogre.get("attached_to").is_none());
    assert_eq!(ogre["attachments"], json!([halo_id]));
    let halo = serde_json::to_value(named(&view, "Halo")).unwrap();
    assert!(halo.get("attachments").is_none() && halo.get("counters").is_none());
    assert_eq!(
        halo["attached_to"],
        json!(driver.id_of("Ogre").unwrap().get())
    );
    let bear = serde_json::to_value(named(&view, "Grizzly Bears")).unwrap();
    assert_eq!(bear["counters"], json!([["+1/+1", 2], ["charge", 1]]));
    // A view that lacks the fields reads back as one without counters or attachments.
    let mut plain = serde_json::to_value(named(&view, "Jace")).unwrap();
    plain.as_object_mut().unwrap().remove("counters");
    let back: ObjectView = serde_json::from_value(plain).unwrap();
    assert_eq!(
        (back.counters, back.attached_to, back.attachments),
        (vec![], None, vec![])
    );
}

#[test]
fn a_host_that_left_or_came_back_holds_nothing() {
    let driver = table(2);
    let mut state = driver.state().clone();
    let ogre = driver.id_of("Ogre").unwrap();
    let before = StateView::for_player(&state, PlayerId(1));
    assert_eq!(named(&before, "Halo").attached_to, Some(ogre));
    let owner = PlayerId(1);
    let leave = Event::ZoneChanged {
        object: ogre,
        from: Zone::Battlefield,
        to: Zone::Graveyard(owner),
        cause: ZoneCause::Destroyed,
    };
    apply_event(&mut state, &leave).unwrap();
    let gone = StateView::for_player(&state, PlayerId(1));
    assert_eq!(named(&gone, "Halo").attached_to, None);
    let in_graveyard = gone.objects.iter().find(|o| o.id == ogre).unwrap();
    assert_eq!(in_graveyard.attachments, []);
    // It returns as a new object: the Halo was attached to the old one.
    let back = Event::ZoneChanged {
        object: ogre,
        from: Zone::Graveyard(owner),
        to: Zone::Battlefield,
        cause: ZoneCause::Moved,
    };
    apply_event(&mut state, &back).unwrap();
    let again = StateView::for_player(&state, PlayerId(1));
    assert_eq!(named(&again, "Halo").attached_to, None);
    assert_eq!(named(&again, "Ogre").attachments, []);
}
