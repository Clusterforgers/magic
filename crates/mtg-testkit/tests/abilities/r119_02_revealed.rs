//! Goal: Firestorm Phoenix that would die returns to its owner's hand instead, where it stays revealed to every seat and
//! can't be cast until its owner's next turn begins, when it is castable and hidden again; at two to four seats, and
//! each game replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::sight::revealed_hand;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Whether the Phoenix is offered to cast at the pending priority decision of seat 0.
fn castable(driver: &Driver) -> bool {
    let Some(&phoenix) = driver.state().zone(Zone::Hand(PlayerId(0))).first() else {
        return false;
    };
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.castable.contains(&phoenix),
        _ => false,
    }
}

#[rstest]
fn the_phoenix_returns_revealed_and_unplayable_until_its_owners_turn(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            std::iter::once(ready(card("Firestorm Phoenix").unwrap()))
                .chain((0..7).map(|_| ready(mountain()))),
        )
        .hand(0, [burn("Blast", 4)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let phoenix = driver.target_named("Firestorm Phoenix").unwrap();
    driver.cast_at("Blast", &[phoenix]).unwrap();
    for _ in 0..20 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass().unwrap();
    }
    let id = *driver
        .state()
        .zone(Zone::Hand(PlayerId(0)))
        .first()
        .unwrap_or_else(|| {
            panic!(
                "{:?}",
                driver
                    .engine()
                    .events()
                    .iter()
                    .rev()
                    .take(12)
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(
        driver.state().object(id).unwrap().card().name,
        "Firestorm Phoenix"
    );
    assert_eq!(revealed_hand(driver.state(), PlayerId(0)), vec![id]);
    assert!(driver.state().is_shown_to(PlayerId(1), id));
    assert!(!castable(&driver), "it can't be played this turn");
    let next = 2 + u32::try_from(seats).unwrap();
    driver.idle_until(next, Step::Main1).unwrap();
    assert_eq!(revealed_hand(driver.state(), PlayerId(0)), Vec::new());
    assert!(castable(&driver), "its owner's next turn frees it");
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}
