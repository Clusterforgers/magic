//! Goal: a seat looks at the top cards of its library by hand and puts them back in any order, on the bottom or into its graveyard as one action, only cards it looked at that are still the top cards, each name once; a draw keeps the rest known, a shuffle forgets them, and a rebuilt engine knows the same cards and reaches the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::error::IllegalAnswer;
use mtg_engine::manual::{ManualAction, ManualRefusal};
use mtg_engine::sight::known_top;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::{name_of, pid};

/// Seat 0 holds priority with a library of A, B, C over two forests, A on top; seat 1 has a library of its own.
fn game() -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .library(
                0,
                [
                    free_creature("A", 1, 1),
                    free_creature("B", 1, 1),
                    free_creature("C", 1, 1),
                    forest(),
                    forest(),
                ],
            )
            .library(1, vec![forest(); 3]),
    )
    .unwrap()
}

/// The ids of the named cards in seat 0's library, in the order named.
fn ids(driver: &Driver, names: &[&str]) -> Vec<ObjectId> {
    names
        .iter()
        .map(|name| {
            *driver
                .state()
                .zone(Zone::Library(pid(0)))
                .iter()
                .find(|id| name_of(driver.state(), **id) == *name)
                .unwrap_or_else(|| panic!("{name} is not in the library"))
        })
        .collect()
}

/// Seat 0's library from the top down, by name.
fn library(driver: &Driver) -> Vec<String> {
    let state = driver.state();
    state
        .zone(Zone::Library(pid(0)))
        .iter()
        .rev()
        .map(|id| name_of(state, *id))
        .collect()
}

fn arrange(driver: &Driver, top: &[&str], bottom: &[&str], graveyard: &[&str]) -> ManualAction {
    ManualAction::ArrangeTop {
        top: ids(driver, top),
        bottom: ids(driver, bottom),
        graveyard: ids(driver, graveyard),
    }
}

/// Takes the action as seat 0 and checks that a rebuilt engine reaches the same state and knows the same cards.
fn take(driver: &mut Driver, action: ManualAction) -> Vec<Event> {
    let events = driver.manual(action).unwrap();
    let rebuilt = Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), driver.state().state_hash());
    assert_eq!(
        known_top(rebuilt.state(), pid(0)),
        known_top(driver.state(), pid(0))
    );
    events
}

fn refusal(driver: &mut Driver, action: ManualAction) -> ManualRefusal {
    let hash = driver.state().state_hash();
    let error = driver.manual(action).unwrap_err();
    assert_eq!(driver.state().state_hash(), hash);
    let ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Manual(refusal))) = error else {
        panic!("not a manual refusal: {error:?}");
    };
    refusal
}

#[rstest]
#[case::reordered(&["C", "A", "B"], &[], &[], &["C", "A", "B", "Forest", "Forest"], 0)]
#[case::one_to_the_bottom(&["B", "C"], &["A"], &[], &["B", "C", "Forest", "Forest", "A"], 0)]
#[case::two_to_the_bottom_in_order(&["C"], &["B", "A"], &[], &["C", "Forest", "Forest", "B", "A"], 0)]
#[case::surveiled(&["C"], &[], &["A", "B"], &["C", "Forest", "Forest"], 2)]
#[case::all_three_ways(&["B"], &["C"], &["A"], &["B", "Forest", "Forest", "C"], 1)]
#[case::all_into_the_graveyard(&[], &[], &["A", "B", "C"], &["Forest", "Forest"], 3)]
fn the_cards_looked_at_go_back_as_the_seat_puts_them(
    #[case] top: &[&str],
    #[case] bottom: &[&str],
    #[case] graveyard: &[&str],
    #[case] expected: &[&str],
    #[case] milled: usize,
) {
    let mut driver = game();
    take(&mut driver, ManualAction::LookTop { count: 3 });
    assert_eq!(
        known_top(driver.state(), pid(0)),
        ids(&driver, &["A", "B", "C"])
    );
    let action = arrange(&driver, top, bottom, graveyard);
    let events = take(&mut driver, action);
    assert!(matches!(events.first(), Some(Event::ManualAction { .. })));
    assert_eq!(library(&driver), expected);
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(0))).len(), milled);
    assert_eq!(known_top(driver.state(), pid(0)).len(), top.len());
}

#[rstest]
#[case::a_card_not_looked_at(&["A", "B", "C"], &[], &[])]
#[case::a_card_named_twice(&["A", "A"], &[], &[])]
#[case::not_the_top_cards(&["B"], &[], &[])]
#[case::too_many_cards(&["A", "B", "C", "Forest"], &[], &[])]
fn only_the_top_cards_looked_at_are_put_back(
    #[case] top: &[&str],
    #[case] bottom: &[&str],
    #[case] graveyard: &[&str],
) {
    let mut driver = game();
    take(&mut driver, ManualAction::LookTop { count: 2 });
    let action = arrange(&driver, top, bottom, graveyard);
    assert_eq!(refusal(&mut driver, action), ManualRefusal::NotLookedAt);
}

#[test]
fn a_card_of_another_library_is_not_put_back() {
    let mut driver = game();
    take(&mut driver, ManualAction::LookTop { count: 1 });
    let theirs = *driver.state().zone(Zone::Library(pid(1))).last().unwrap();
    let action = ManualAction::ArrangeTop {
        top: vec![theirs],
        bottom: Vec::new(),
        graveyard: Vec::new(),
    };
    assert_eq!(refusal(&mut driver, action), ManualRefusal::NotLookedAt);
}

#[test]
fn a_draw_keeps_the_rest_known_and_a_shuffle_forgets_them() {
    let mut driver = game();
    take(&mut driver, ManualAction::LookTop { count: 3 });
    take(&mut driver, ManualAction::Draw { count: 1 });
    assert_eq!(known_top(driver.state(), pid(0)), ids(&driver, &["B", "C"]));
    let back = arrange(&driver, &["C"], &["B"], &[]);
    take(&mut driver, back);
    assert_eq!(library(&driver)[0], "C");
    let events = take(&mut driver, ManualAction::Shuffle {});
    assert!(matches!(events[1], Event::LibraryShuffled { .. }));
    assert_eq!(known_top(driver.state(), pid(0)), Vec::<ObjectId>::new());
    let after = arrange(&driver, &[], &[], &["C"]);
    assert_eq!(refusal(&mut driver, after), ManualRefusal::NotLookedAt);
}

#[test]
fn a_shuffle_is_the_same_on_a_replay_and_spends_the_random_stream() {
    let mut driver = game();
    let before = driver.state().rng_index();
    take(&mut driver, ManualAction::Shuffle {});
    assert_eq!(driver.state().rng_index(), before + 1);
    assert_eq!(library(&driver).len(), 5);
}
