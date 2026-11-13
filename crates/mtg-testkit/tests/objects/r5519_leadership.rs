//! Goal: "Target player returns each commander they control from the battlefield to the command zone."
//! (`Destination::Command`, Leadership Vacuum) puts only the target player's commanders into their owner's command
//! zone, leaves their other creatures and everyone else's commanders, and the caster draws, at two to four seats; the
//! log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use crate::common::pid;
use mtg_core::color::Color;
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Commander;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island, mountain};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Passes priority until the stack is empty.
fn r5519_settle(driver: &mut Driver) {
    for _ in 0..30 {
        if driver.state().stack().is_empty()
            || !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
        {
            break;
        }
        driver.pass().unwrap();
    }
}

/// The zone of the one object named `name`.
fn r5519_zone(driver: &Driver, name: &str) -> Zone {
    let found = driver
        .state()
        .objects()
        .filter(|object| object.card().name == name)
        .map(|object| object.zone())
        .collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "{name}: {found:?}");
    found[0]
}

#[rstest]
fn r5519_leadership_vacuum_returns_only_the_targets_commanders(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] target_self: bool,
) {
    let mut mine = lands(3, mountain);
    mine.extend(lands(3, island));
    mine.push(ready(free_creature("Bystander", 2, 2)));
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .format(Arc::new(Commander))
            .at(3, 0, Step::Main1)
            .commanders(0, [legend("Cmd", 2, &[Color::Red], 3, 3)])
            .hand(0, [card("Leadership Vacuum").unwrap()])
            .battlefield(0, mine),
        |scenario, seat| scenario.library(seat, (0..6).map(|_| forest())),
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver
        .cast_tapping("Cmd", &["Mountain", "Mountain", "Mountain"])
        .unwrap();
    r5519_settle(&mut driver);
    assert_eq!(r5519_zone(&driver, "Cmd"), Zone::Battlefield);
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let target = driver.target_player(if target_self { 0 } else { seats - 1 });
    driver.cast_at("Leadership Vacuum", &[target]).unwrap();
    r5519_settle(&mut driver);
    let expected = if target_self {
        Zone::Command(pid(0))
    } else {
        Zone::Battlefield
    };
    assert_eq!(r5519_zone(&driver, "Cmd"), expected);
    assert_eq!(r5519_zone(&driver, "Bystander"), Zone::Battlefield);
    // The spell left the hand and its controller drew a card.
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand);
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
