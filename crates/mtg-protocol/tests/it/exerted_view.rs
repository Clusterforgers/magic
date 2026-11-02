//! Goal: that an exerted creature reads as exerted in the permanent view of every seat until the untap step that ends it, and one not exerted never does, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::turn::Step;
use mtg_protocol::StateView;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Answers passively until the pending decision is `kind` for seat 0.
fn until(driver: &mut Driver, kind: &str) {
    for _ in 0..400 {
        let decision = driver.decision().unwrap().clone();
        if decision.player == PlayerId(0) && decision.kind.name() == kind {
            return;
        }
        driver.answer(Driver::passive_answer(&decision)).unwrap();
    }
    panic!("no {kind} decision");
}

#[rstest]
fn an_exerted_creature_is_shown_exerted_to_every_seat(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] exert: bool,
) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(card("Glorybringer").unwrap())])
        .battlefield(1, [ready(free_creature("Bear", 2, 2))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let glorybringer = driver.id_of("Glorybringer").unwrap();
    until(&mut driver, "declare-attackers");
    let exerted_in = |driver: &Driver, viewer: usize| {
        let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
        let shown = view.objects.iter().find(|o| o.id == glorybringer).unwrap();
        shown.permanent.as_ref().unwrap().exerted
    };
    assert!((0..seats).all(|viewer| !exerted_in(&driver, viewer)));
    if exert {
        driver
            .w11a_attack_exerting(&[("Glorybringer", 1)], &["Glorybringer"])
            .unwrap();
        until(&mut driver, "choose-targets");
        let at = driver.target_named("Bear").unwrap();
        driver.choose_targets(&[at]).unwrap();
    } else {
        driver.attack(&[("Glorybringer", 1)]).unwrap();
    }
    driver.idle_until(2, Step::Main2).unwrap();
    assert!((0..seats).all(|viewer| exerted_in(&driver, viewer) == exert));
}
