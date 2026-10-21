//! Goal: a foretold card lies in exile for every seat to see that it is there, and only its owner sees which card it is: the other seats and a spectator get an object with no characteristics, the move is told to them without the card, the log never names it to them while it lies face down, and it is named to everyone the moment it is cast or leaves exile for a place the viewer sees into.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, PayChoice, PriorityAction};
use mtg_engine::events::ZoneCause;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventBatch, EventView, HiddenEvent, ObjectView, StateView, project_events};
use mtg_testkit::cards::{forest, island};
use mtg_testkit::{Driver, Scenario, foretell_fuzz_setup, ready};
use rstest::rstest;
use serde_json::Value;

use crate::common::{run_game, with_unique_names};
use crate::hidden_info::{assert_clean, hidden_in};

const CARD: &str = "Behold the Multiverse";

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Every viewer of a table of `seats`: each seat and one spectator.
fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..=seats).map(PlayerId::from_index).collect()
}

/// Seat 0 holds the foretell card and two cards that stay hidden; every other seat has a hand of its own.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 6])
        .hand_named(0, [CARD, "Counterspell", "Wrath of God"])
        .battlefield(0, (0..5).map(|_| ready(island())).collect::<Vec<_>>());
    let others = [
        ["Serra Angel", "Doom Blade"],
        ["Fireball", "Giant Growth"],
        ["Opt", "Brainstorm"],
    ];
    for other in 1..seats {
        scenario = scenario.hand_named(other, others[other - 1]);
    }
    scenario
}

/// Whether the text of `value` holds `name` anywhere.
fn names(value: &Value, name: &str) -> bool {
    match value {
        Value::String(text) => text == name,
        Value::Array(items) => items.iter().any(|item| names(item, name)),
        Value::Object(map) => map.values().any(|item| names(item, name)),
        _ => false,
    }
}

/// Whether any part of the batch holds the card's name.
fn batch_names(batch: &EventBatch, name: &str) -> bool {
    [
        serde_json::to_value(&batch.events).unwrap(),
        serde_json::to_value(&batch.table).unwrap(),
        serde_json::to_value(&batch.objects).unwrap(),
        serde_json::to_value(&batch.gone).unwrap(),
    ]
    .iter()
    .any(|part| names(part, name))
}

fn object_of(batch: &EventBatch, id: ObjectId) -> Option<&ObjectView> {
    batch.objects.iter().find(|object| object.id == id)
}

/// Foretells the card from seat 0's hand by paying with the islands.
fn foretell(driver: &mut Driver) -> ObjectId {
    let id = driver
        .state()
        .zone(Zone::Hand(seat(0)))
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == CARD)
        .unwrap();
    driver
        .answer(Answer::Priority(PriorityAction::Foretell(id)))
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    id
}

#[rstest]
fn only_the_owner_sees_which_card_was_foretold(#[values(2, 3, 4)] seats: usize) {
    let scenario = table(seats);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let id = foretell(&mut driver);
    let events = driver.engine().events().to_vec();
    let after = driver.state().clone();
    let all = viewers(seats);
    let batches = project_events(&before, &events, &all, 0).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        let owner = *viewer == seat(0);
        let object = object_of(batch, id).expect("every viewer sees that the card is in exile");
        assert_eq!(object.zone, Zone::Exile);
        assert_eq!(object.owner, seat(0));
        assert!(object.face_down, "the card lies face down for {viewer:?}");
        assert!(
            batch.table.exile.cards.as_ref().unwrap().contains(&id),
            "it is listed in exile for {viewer:?}"
        );
        assert_eq!(
            batch_names(batch, CARD),
            owner,
            "who is told the name: {viewer:?}"
        );
        if owner {
            assert_eq!(object.name, CARD);
            assert_eq!(object.types, [CardType::Instant]);
            assert!(batch.events.iter().any(|event| matches!(
                event,
                EventView::Public(Event::ZoneChanged { object, cause: ZoneCause::Foretold, .. })
                    if *object == id
            )));
        } else {
            assert_eq!(object.name, "");
            assert!(object.types.is_empty() && object.abilities.is_empty());
            assert!(object.colors.is_empty() && object.mana_cost.is_none());
            assert!(object.keywords.is_empty() && object.counters.is_empty());
            assert_eq!(object.power, None);
            assert!(batch.events.iter().any(|event| matches!(
                event,
                EventView::Hidden(HiddenEvent::ZoneChanged {
                    from: Zone::Hand(from),
                    to: Zone::Exile,
                    cause: ZoneCause::Foretold,
                }) if *from == seat(0)
            )));
            assert!(
                !batch.events.iter().any(|event| matches!(
                    event,
                    EventView::Public(Event::ZoneChanged { object, .. }) if *object == id
                )),
                "the move is not told with the card to {viewer:?}"
            );
            assert_clean(batch, &hidden_in(&after, *viewer));
        }
    }
    // The state a client resumes from is told the same way.
    for viewer in &all {
        let view = StateView::for_player(&after, *viewer);
        let object = view.objects.iter().find(|object| object.id == id).unwrap();
        assert_eq!(object.name == CARD, *viewer == seat(0));
        assert_eq!(
            names(&serde_json::to_value(&view).unwrap(), CARD),
            *viewer == seat(0)
        );
    }
}

#[rstest]
fn the_card_is_named_to_every_seat_as_it_is_cast(#[values(2, 3, 4)] seats: usize) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let id = foretell(&mut driver);
    driver.idle_until(2, Step::Upkeep).unwrap();
    while driver.decision().unwrap().player != seat(0) {
        driver.pass().unwrap();
    }
    let before = driver.state().clone();
    let logged = driver.engine().events().len();
    let from_seq = u64::try_from(logged).unwrap();
    driver
        .answer(Answer::Priority(PriorityAction::CastSpell(id)))
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let events = driver.engine().events()[logged..].to_vec();
    let all = viewers(seats);
    let batches = project_events(&before, &events, &all, from_seq).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::SpellCast { object, .. }) if *object == id
            )),
            "{viewer:?} sees the cast"
        );
        let object = object_of(batch, id).expect("the spell is on the stack for everyone");
        assert_eq!(object.zone, Zone::Stack);
        assert_eq!(object.name, CARD, "{viewer:?} reads the spell");
        assert!(!object.face_down, "the spell is face up");
    }
}

#[rstest]
fn the_card_is_named_to_every_seat_as_it_leaves_exile_for_a_graveyard(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let id = foretell(&mut driver);
    let before = driver.state().clone();
    let event = Event::ZoneChanged {
        object: id,
        from: Zone::Exile,
        to: Zone::Graveyard(seat(0)),
        cause: ZoneCause::Moved,
    };
    let all = viewers(seats);
    let batches = project_events(&before, &[event], &all, 0).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::ZoneChanged { object, .. }) if *object == id
            )),
            "{viewer:?} sees the card go to the graveyard"
        );
        let object = object_of(batch, id).expect("the card is in the open");
        assert_eq!(object.name, CARD);
        assert!(!object.face_down);
    }
}

#[rstest]
#[case::to_the_hand(Zone::Hand(seat(0)))]
#[case::to_the_library(Zone::Library(seat(0)))]
fn a_foretold_card_that_goes_to_a_hidden_place_is_told_without_the_card(
    #[case] to: Zone,
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let id = foretell(&mut driver);
    let before = driver.state().clone();
    let event = Event::ZoneChanged {
        object: id,
        from: Zone::Exile,
        to,
        cause: ZoneCause::Moved,
    };
    let all = viewers(seats);
    let batches = project_events(&before, &[event], &all, 0).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        let owner = *viewer == seat(0);
        let told = batch.events.iter().any(|view| {
            matches!(
                view,
                EventView::Public(Event::ZoneChanged { object, .. }) if *object == id
            )
        });
        // The owner knew the card in exile; no other seat sees where it went.
        assert_eq!(told, owner, "{viewer:?}");
        if !owner {
            assert!(
                batch.events.iter().any(|view| matches!(
                    view,
                    EventView::Hidden(HiddenEvent::ZoneChanged {
                        from: Zone::Exile,
                        ..
                    })
                )),
                "{viewer:?} is told that a card left exile"
            );
            assert!(
                batch.gone.contains(&id),
                "the face-down object is gone for {viewer:?}"
            );
            assert!(!batch_names(batch, CARD));
        }
    }
}

/// The names of the cards that lie face down in `state` and that `viewer` may not look at.
fn face_down_names(state: &GameState, viewer: PlayerId) -> Vec<String> {
    state
        .objects()
        .filter(|o| o.face_down().is_some() && !state.may_look_at(viewer, o.id()))
        .map(|o| o.card().name.clone())
        .collect()
}

/// Games with the foretell cards under unique names: no part of what a seat is told names a card that lies face down
/// after the answer and that the seat may not look at.
#[rstest]
#[case(2, 31)]
#[case(3, 32)]
#[case(4, 33)]
fn no_face_down_card_reaches_a_seat_that_may_not_look_at_it(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let all = viewers(players);
    let (mut foretold, mut checked) = (0usize, 0usize);
    let setup = with_unique_names(foretell_fuzz_setup(players, seed));
    let engine = run_game(setup, seed, |step| {
        foretold += step
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Event::ZoneChanged {
                        cause: ZoneCause::Foretold,
                        ..
                    }
                )
            })
            .count();
        let batches = project_events(step.before, step.events, &all, step.from_seq).unwrap();
        for (viewer, batch) in all.iter().zip(&batches) {
            for name in face_down_names(step.after, *viewer) {
                checked += 1;
                assert!(
                    !batch_names(batch, &name),
                    "seed {seed} answer {} viewer {viewer:?}: the batch names {name}",
                    step.answers
                );
            }
            let view = StateView::for_player(step.after, *viewer);
            for name in face_down_names(step.after, *viewer) {
                assert!(
                    !names(&serde_json::to_value(&view).unwrap(), &name),
                    "seed {seed} answer {} viewer {viewer:?}: the view names {name}",
                    step.answers
                );
            }
        }
    });
    assert!(engine.result().is_some(), "the game finishes");
    assert!(
        foretold > 0 && checked > 0,
        "the test is not vacuous: {foretold} {checked}"
    );
}
