//! Goal: while a seat controls another (CR 722) it sees the controlled seat's hand and the turn names it as the controller, every other seat and a spectator see neither the hand nor more than the public control, the scheduling event reaches every viewer whole, and once the turn is over the hand is hidden again, at 2, 3 and 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_protocol::{EventView, StateView, project_events};
use mtg_testkit::cards::free_creature;
use mtg_testkit::control_cards::mindslave;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

fn hand_seen(view: &StateView, of: usize) -> Option<usize> {
    view.table.players[of].hand.cards.as_ref().map(Vec::len)
}

#[rstest]
fn the_controller_sees_the_controlled_hand_for_that_turn(#[values(2, 3, 4)] seats: usize) {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [mindslave("Mindslave", false)]);
    for n in 0..seats {
        scenario = scenario
            .library(n, (0..5).map(|k| free_creature(&format!("Deep {n}.{k}"), 1, 1)));
    }
    for n in 1..seats {
        scenario = scenario.hand(n, [free_creature(&format!("Secret {n}"), 1, 1)]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_player(1);
    let before = driver.state().clone();
    let events = driver.cast_at("Mindslave", &[target]).unwrap();
    let mut all = events;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        all.extend(driver.answer_passively().unwrap());
    }
    // The scheduling is public.
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    for batch in project_events(&before, &all, &viewers, 0).unwrap() {
        assert!(batch.events.iter().any(|view| matches!(
            view,
            EventView::Public(Event::PlayerControlGained { player, by, .. })
                if *player == seat(1) && *by == seat(0)
        )));
    }
    driver.idle_until(2, Step::Main1).unwrap();
    let state = driver.state();
    let mine = StateView::for_player(state, seat(0));
    assert!(hand_seen(&mine, 1).is_some_and(|n| n > 0));
    assert_eq!(mine.table.turn.controlled_by, Some(seat(0)));
    for n in 2..seats {
        let theirs = StateView::for_player(state, seat(n));
        assert_eq!(hand_seen(&theirs, 1), None);
        assert_eq!(theirs.table.turn.controlled_by, Some(seat(0)));
    }
    assert_eq!(hand_seen(&StateView::for_spectator(state), 1), None);
    driver.idle_until(3, Step::Main1).unwrap();
    let after = StateView::for_player(driver.state(), seat(0));
    assert_eq!(hand_seen(&after, 1), None);
    assert_eq!(after.table.turn.controlled_by, None);
}
