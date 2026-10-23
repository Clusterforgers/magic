//! Goal: at two, three and four seats a companion is named to every seat as existing but its card only to its owner until it is revealed, then to every seat; the reveal and the bringing into the hand reach every seat whole, the card in the hand is an object only its owner may identify, the option to bring it is on the wire only while it is offered, and a table view with companions round-trips through its JSON.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_core::ids::PlayerId;
use mtg_engine::companions::CompanionStage;
use mtg_engine::decision::{DecisionKind, PriorityAction};
use mtg_engine::format::Normal;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, GameState, apply_event};
use mtg_protocol::{EventView, TableView, project_events};
use mtg_testkit::cards::forest;
use mtg_testkit::companion_cards::{small_companion, small_deck};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// A game that has not started, with a hidden companion at every seat but the last.
fn unrevealed(seats: usize) -> GameState {
    let decks = (0..seats)
        .map(|n| small_deck(&mut mtg_engine::rng::GameRng::substream(3, n as u64)))
        .collect();
    let setup = GameSetup::new(Arc::new(Normal), 3, decks).with_companions(
        (0..seats)
            .map(|n| (n + 1 < seats).then(small_companion))
            .collect(),
    );
    GameState::from_setup(&setup).unwrap()
}

#[rstest]
fn a_hidden_companion_is_named_to_its_owner_only(#[values(2, 3, 4)] seats: usize) {
    let state = unrevealed(seats);
    for viewer in 0..seats {
        let view = TableView::of(&state, seat(viewer));
        for owner in 0..seats {
            let json = serde_json::to_value(&view.players[owner]).unwrap();
            if owner + 1 == seats {
                assert!(view.players[owner].companion.is_none());
                assert!(json.get("companion").is_none(), "left out without one");
                continue;
            }
            let companion = view.players[owner].companion.as_ref().unwrap();
            assert_eq!(companion.stage, CompanionStage::Hidden);
            assert_eq!(companion.object, None);
            if viewer == owner {
                assert_eq!(companion.name.as_deref(), Some("Small Companion"));
            } else {
                assert_eq!(
                    companion.name, None,
                    "seat {viewer} must not see seat {owner}'s card"
                );
                assert!(json["companion"].get("name").is_none());
            }
        }
        let json = serde_json::to_value(&view).unwrap();
        let back: TableView = serde_json::from_value(json).unwrap();
        assert_eq!(back, view);
    }
}

#[rstest]
fn the_reveal_names_the_card_to_every_seat_and_reaches_them_whole(#[values(2, 3, 4)] seats: usize) {
    let mut state = unrevealed(seats);
    let before = state.clone();
    let event = Event::CompanionRevealed { player: seat(0) };
    apply_event(&mut state, &event).unwrap();
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    for viewer in &viewers {
        let companion = TableView::of(&state, *viewer).players[0]
            .companion
            .clone()
            .unwrap();
        assert_eq!(companion.stage, CompanionStage::Revealed);
        assert_eq!(companion.name.as_deref(), Some("Small Companion"));
    }
    let others = TableView::of(&state, seat(0));
    if seats > 2 {
        assert_eq!(
            others.players[1].companion.as_ref().unwrap().name,
            None,
            "a companion that is not revealed stays hidden"
        );
    }
    let batches = project_events(&before, std::slice::from_ref(&event), &viewers, 0).unwrap();
    for batch in &batches {
        assert!(
            batch
                .events
                .iter()
                .any(|told| matches!(told, EventView::Public(seen) if *seen == event))
        );
        assert!(
            batch
                .objects
                .iter()
                .all(|object| object.name != "Small Companion"),
            "a card outside the game is no object"
        );
    }
}

#[rstest]
fn the_card_in_the_hand_is_an_object_only_its_owner_identifies(#[values(2, 3, 4)] seats: usize) {
    let mut scenario = Scenario::new(seats).battlefield(0, vec![ready(forest()); 3]);
    for n in 0..seats {
        scenario = scenario.companion(n, small_companion());
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let before = driver.state().clone();
    let seq = driver.engine().events().len();
    driver
        .answer(mtg_engine::decision::Answer::Priority(
            PriorityAction::BringCompanion,
        ))
        .unwrap();
    driver
        .pay_tapping(&["Forest", "Forest", "Forest"], 0)
        .unwrap();
    let events = driver.engine().events()[seq..].to_vec();
    let brought = events
        .iter()
        .find_map(|event| match event {
            Event::CompanionBrought { object, .. } => Some(*object),
            _ => None,
        })
        .unwrap();
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(&before, &events, &viewers, seq as u64).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        assert!(
            batch.events.iter().any(|told| matches!(
                told,
                EventView::Public(Event::CompanionBrought { player, object })
                    if *player == seat(0) && *object == brought
            )),
            "every seat is told that seat 0 brought its companion"
        );
        let hand = &batch.table.players[0].hand;
        assert_eq!(
            hand.count,
            1 + u32::try_from(before.zone(Zone::Hand(seat(0))).len()).unwrap()
        );
        let companion = batch.table.players[0].companion.as_ref().unwrap();
        assert_eq!(companion.stage, CompanionStage::Brought);
        assert_eq!(companion.object, Some(brought));
        assert_eq!(companion.name.as_deref(), Some("Small Companion"));
        let sees_the_card = batch.objects.iter().any(|object| object.id == brought);
        assert_eq!(sees_the_card, *viewer == seat(0), "viewer {viewer:?}");
        assert_eq!(hand.cards.is_some(), *viewer == seat(0));
    }
}

#[rstest]
fn the_option_is_on_the_wire_only_while_it_is_offered(
    #[values(2, 3, 4)] seats: usize,
    #[values(Step::Main1, Step::Upkeep)] step: Step,
) {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, step)
        .battlefield(0, vec![ready(forest()); 3]);
    for n in 0..seats {
        scenario = scenario.companion(n, small_companion());
    }
    let driver = Driver::from_scenario(scenario).unwrap();
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority is asked");
    };
    let json = serde_json::to_value(options).unwrap();
    assert_eq!(options.companion, step == Step::Main1);
    assert_eq!(
        json.get("companion"),
        (step == Step::Main1).then_some(&serde_json::json!(true))
    );
    let back: mtg_engine::decision::PriorityOptions = serde_json::from_value(json).unwrap();
    assert_eq!(&back, &**options);
    assert_eq!(
        serde_json::to_string(&PriorityAction::BringCompanion).unwrap(),
        r#""BringCompanion""#
    );
}
