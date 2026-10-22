//! Goal: every seat sees the command zone and the commanders of every seat (the cards, the times each was cast from the command zone and the tax it carries) and the combat damage each commander has dealt to each seat, hands stay hidden, the new fields are left out of the JSON outside Commander and while nothing is counted, and the commander's choice to stay and the loss for commander damage reach every seat as events.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::Answer;
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::player::LossReason;
use mtg_engine::turn::Step;
use mtg_engine::{Event, apply_event};
use mtg_protocol::{EventView, StateView, project_events};
use mtg_testkit::cards::{bear, mountain};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 owns the commander "Cmd" and has lands to cast it twice; the table has `seats` seats.
fn table(seats: usize) -> Driver {
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 1, &[Color::Red], 2, 2)])
        .battlefield(0, (0..8).map(|_| ready(mountain())))
        .battlefield(1, [ready(bear())]);
    Driver::from_scenario(scenario).unwrap()
}

fn cast(driver: &mut Driver, seats: usize) {
    driver.cast("Cmd").unwrap();
    for _ in 0..seats {
        driver.pass().unwrap();
    }
}

fn move_by_hand(driver: &mut Driver, to: ManualZone) {
    let object = driver.state().commanders().next().unwrap();
    driver.manual(ManualAction::Move { object, to }).unwrap();
}

#[rstest]
fn every_seat_sees_the_command_zone_and_the_commanders(
    #[values(2, 3, 6)] seats: usize,
    #[values(0, 1)] viewer: usize,
) {
    let driver = table(seats);
    let id = driver.state().commanders().next().unwrap();
    let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
    let owner = &view.table.players[0];
    assert_eq!(owner.command.cards, Some(vec![id]));
    assert_eq!(owner.command.count, 1);
    assert!(
        view.objects.iter().any(|object| object.id == id),
        "the card is identified"
    );
    assert_eq!(owner.commanders.len(), 1);
    let commander = &owner.commanders[0];
    assert_eq!(
        (commander.object, commander.casts, commander.tax),
        (id, 0, 0)
    );
    for other in &view.table.players[1..] {
        assert!(other.commanders.is_empty() && other.commander_damage.is_empty());
    }
    if viewer != 0 {
        assert_eq!(owner.hand.cards, None, "the hand stays hidden");
    }
}

#[rstest]
#[case::once(1, 2)]
#[case::twice(2, 4)]
fn the_tax_in_the_view_follows_the_casts(#[case] casts: u32, #[case] tax: u32) {
    let mut driver = table(3);
    for _ in 0..casts {
        cast(&mut driver, 3);
        move_by_hand(&mut driver, ManualZone::Graveyard);
        driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
    }
    for viewer in 0..3 {
        let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
        let commander = &view.table.players[0].commanders[0];
        assert_eq!((commander.casts, commander.tax), (casts, tax));
    }
}

#[test]
fn commander_damage_is_in_the_view_of_the_player_it_was_dealt_to() {
    let scenario = Scenario::new(3)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 0, &[], 7, 7)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast(&mut driver, 3);
    driver.idle_until(4, Step::DeclareAttackers).unwrap();
    driver.attack(&[("Cmd", 2)]).unwrap();
    driver.idle_until(4, Step::EndCombat).unwrap();
    let id = driver.state().commanders().next().unwrap();
    let view = StateView::for_player(driver.state(), PlayerId(1));
    let hit = &view.table.players[2];
    assert_eq!(hit.commander_damage.len(), 1);
    let damage = &hit.commander_damage[0];
    assert_eq!((damage.commander, damage.damage), (id, 7));
    assert_eq!(view.table.players[1].commander_damage, []);
    assert_eq!(hit.life, 33);
}

#[test]
fn the_json_leaves_the_new_fields_out_until_they_are_used() {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(bear())])).unwrap();
    let view = serde_json::to_value(StateView::for_player(driver.state(), PlayerId(1))).unwrap();
    for seat in view["players"].as_array().unwrap() {
        assert!(seat.get("commanders").is_none() && seat.get("commander_damage").is_none());
    }
    let driver = table(2);
    let view = serde_json::to_value(StateView::for_player(driver.state(), PlayerId(1))).unwrap();
    let players = view["players"].as_array().unwrap();
    assert!(players[0].get("commanders").is_some());
    assert!(players[0].get("commander_damage").is_none());
}

#[test]
fn the_refusal_and_the_loss_reach_every_seat() {
    let mut driver = table(3);
    cast(&mut driver, 3);
    move_by_hand(&mut driver, ManualZone::Exile);
    let before = driver.state().clone();
    let start = driver.engine().events().len();
    driver.answer(Answer::YesNo(YesNoChoice(false))).unwrap();
    let events = driver.engine().events()[start..].to_vec();
    let object = driver.state().commanders().next().unwrap();
    let viewers = [PlayerId(0), PlayerId(1), PlayerId(2)];
    for batch in project_events(&before, &events, &viewers, 0).unwrap() {
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::CommanderStayed { object: stayed }) if *stayed == object
            )),
            "{:?}",
            batch.events
        );
    }
    let loss = Event::PlayerLost {
        player: PlayerId(1),
        reason: LossReason::CommanderDamage,
    };
    let mut after = before.clone();
    apply_event(&mut after, &loss).unwrap();
    for batch in project_events(&before, std::slice::from_ref(&loss), &viewers, 0).unwrap() {
        assert!(matches!(batch.events.as_slice(), [EventView::Public(event)] if *event == loss));
    }
}
