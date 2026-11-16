//! Goal: a mana ability activated at priority like any other ability (Vexing Puzzlebox's mana and d20 roll, Skirge Familiar's discard) is marked `announced` beside `mana` with the address `PriorityOptions.abilities` offers, while a mana ability the payment taps (a painland, a mana rock, a plain land) is not, at two and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_protocol::{ActivatedView, StateView};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// The views of the named permanent's activated abilities as seat 0 sees them.
fn views(driver: &Driver, name: &str) -> Vec<ActivatedView> {
    let view = StateView::for_player(driver.state(), PlayerId::from_index(0));
    view.objects
        .iter()
        .find(|object| object.name == name)
        .map(|object| object.activated.clone())
        .unwrap_or_default()
}

#[rstest]
#[case::puzzlebox_two_seats("Vexing Puzzlebox", 2, &[Some(true), Some(false)])]
#[case::puzzlebox_four_seats("Vexing Puzzlebox", 4, &[Some(true), Some(false)])]
#[case::skirge_familiar("Skirge Familiar", 2, &[Some(true)])]
#[case::painland("Battlefield Forge", 4, &[None, None])]
#[case::sol_ring("Sol Ring", 2, &[None])]
fn a_mana_ability_activated_at_priority_is_announced(
    #[case] name: &str,
    #[case] seats: usize,
    // For each activated ability in order: `Some(true)` for an announced mana ability, `None` for a mana ability
    // the payment taps, `Some(false)` for any other ability.
    #[case] expected: &[Option<bool>],
) {
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Forest"])
            .battlefield(0, [ready(card(name).unwrap())]),
    )
    .unwrap();
    let views = views(&driver, name);
    let marks: Vec<Option<bool>> = views
        .iter()
        .map(|view| match (view.mana, view.announced) {
            (true, true) => Some(true),
            (true, false) => None,
            (false, announced) => {
                assert!(!announced, "only a mana ability is announced");
                Some(false)
            }
        })
        .collect();
    assert_eq!(marks, expected, "{views:?}");
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("seat 0 has priority");
    };
    for view in views.iter().filter(|view| view.announced) {
        let address = view.ability.expect("an announced ability has its address");
        assert!(
            options.abilities.contains(&address),
            "the priority offer lists {address:?}"
        );
    }
}
