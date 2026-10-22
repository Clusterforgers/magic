//! Goal: at a Commander table of 2 to 6 seats every viewer, a spectator included, sees the commanders of every seat (a pair too), the objects behind them, the times each was cast and the tax it carries, and the combat damage each commander dealt to each seat, while the hands stay hidden from everyone but their owner.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::format::Commander;
use mtg_engine::turn::Step;
use mtg_protocol::StateView;
use mtg_testkit::cards::{bear, mountain};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Every seat leads with "Cmd <seat>", seat 1 with a pair; seat 0 has lands to cast its commander.
fn table(seats: usize) -> Driver {
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .battlefield(0, (0..8).map(|_| ready(mountain())))
        .hand(0, [bear()]);
    for seat in 0..seats {
        let mut leaders = vec![legend(&format!("Cmd {seat}"), 1, &[Color::Red], 2, 2)];
        if seat == 1 {
            leaders.push(legend("Cmd 1b", 1, &[Color::Red], 2, 2));
        }
        scenario = scenario.commanders(seat, leaders);
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Every viewer of a table of `seats`: each seat and one spectator.
fn viewers(driver: &Driver, seats: usize) -> Vec<StateView> {
    (0..=seats)
        .map(|viewer| StateView::for_player(driver.state(), PlayerId::from_index(viewer)))
        .collect()
}

#[rstest]
fn every_viewer_sees_every_seats_commanders_and_the_objects_behind_them(
    #[values(2, 3, 4, 6)] seats: usize,
) {
    let driver = table(seats);
    for (viewer, view) in viewers(&driver, seats).iter().enumerate() {
        assert_eq!(view.table.players.len(), seats);
        for (seat, player) in view.table.players.iter().enumerate() {
            let wanted = if seat == 1 { 2 } else { 1 };
            assert_eq!(
                player.commanders.len(),
                wanted,
                "viewer {viewer}, seat {seat}"
            );
            assert_eq!(player.command.count as usize, wanted);
            let zone = player.command.cards.clone().unwrap();
            for commander in &player.commanders {
                assert!(zone.contains(&commander.object));
                let object = view
                    .objects
                    .iter()
                    .find(|object| object.id == commander.object)
                    .unwrap_or_else(|| {
                        panic!("viewer {viewer} cannot see a commander of seat {seat}")
                    });
                assert!(object.name.starts_with("Cmd"), "{}", object.name);
                assert_eq!((commander.casts, commander.tax), (0, 0));
            }
            assert_eq!(player.commander_damage, []);
            // Only the owner sees which cards are in a hand.
            assert_eq!(
                player.hand.cards.is_some(),
                viewer == seat,
                "viewer {viewer}, seat {seat}"
            );
        }
    }
}

#[rstest]
fn the_tax_reaches_every_viewer_the_same(#[values(2, 3, 6)] seats: usize) {
    let mut driver = table(seats);
    driver.cast("Cmd 0").unwrap();
    for _ in 0..seats {
        driver.pass().unwrap();
    }
    let id = driver.state().commanders().next().unwrap();
    let all = viewers(&driver, seats);
    let reference = all[0].table.players[0].commanders.clone();
    assert_eq!(reference.len(), 1);
    assert_eq!((reference[0].object, reference[0].casts), (id, 1));
    for (viewer, view) in all.iter().enumerate() {
        assert_eq!(
            view.table.players[0].commanders, reference,
            "viewer {viewer}"
        );
    }
}

#[rstest]
fn commander_damage_is_told_to_a_spectator_as_to_the_players(#[values(3, 4, 6)] seats: usize) {
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 0, &[], 7, 7)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Cmd").unwrap();
    for _ in 0..seats {
        driver.pass().unwrap();
    }
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::DeclareAttackers).unwrap();
    driver.attack(&[("Cmd", seats - 1)]).unwrap();
    driver.idle_until(turn, Step::EndCombat).unwrap();
    let id = driver.state().commanders().next().unwrap();
    for viewer in 0..=seats {
        let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
        let hit = &view.table.players[seats - 1];
        assert_eq!(hit.commander_damage.len(), 1, "viewer {viewer}");
        let damage = &hit.commander_damage[0];
        assert_eq!(
            (damage.commander, damage.damage),
            (id, 7),
            "viewer {viewer}"
        );
        assert_eq!(hit.life, 33);
        assert!(
            view.table.players[1..seats - 1]
                .iter()
                .all(|player| player.commander_damage.is_empty())
        );
    }
}
