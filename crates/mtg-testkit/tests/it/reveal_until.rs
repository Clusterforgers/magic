//! Goal: revealing until a card matches plays whole through Treasure Hunt and Clifftop Lookout at two, three and four seats: every card down to the first match is revealed to every seat in one event and no event names a card below it, the cards go where the card says (all to the hand, or the match onto the battlefield tapped and the rest to the bottom), a library with no match is revealed whole, and the game replays from its log to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ops::Range;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::library::Revealed;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{creature, forest, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::{names, pid};

/// A library from a spec, top first: `l` is a Forest, `c` a creature named for its place.
fn library_of(spec: &str) -> Vec<Arc<CardDefinition>> {
    spec.chars()
        .enumerate()
        .map(|(at, kind)| match kind {
            'l' => forest(),
            _ => creature(&format!("Creature {at}"), 9, &[], 1, 1),
        })
        .collect()
}

/// The names of the cards of a spec at some places.
fn named(spec: &str, range: Range<usize>) -> Vec<String> {
    spec.chars()
        .enumerate()
        .filter(|(at, _)| range.contains(at))
        .map(|(at, kind)| match kind {
            'l' => "Forest".to_owned(),
            _ => format!("Creature {at}"),
        })
        .collect()
}

/// The names in seat 0's library, top card first.
fn top_first(state: &mtg_engine::GameState) -> Vec<String> {
    let mut library = names(state, Zone::Library(pid(0)));
    library.reverse();
    library
}

fn sorted(mut cards: Vec<String>) -> Vec<String> {
    cards.sort();
    cards
}

/// The `Revealed` events among the events.
fn revealed(events: &[Event]) -> Vec<Revealed> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed(revealed) => Some(revealed.clone()),
            _ => None,
        })
        .collect()
}

/// Casts the card and passes until the stack is empty again: the spell and what it triggers resolve.
fn play(driver: &mut Driver, name: &str) -> Result<Vec<Event>, ScriptError> {
    let start = driver.engine().events().len();
    driver.cast(name)?;
    while !driver.state().zone(Zone::Stack).is_empty() {
        driver.pass()?;
    }
    Ok(driver.engine().events()[start..].to_vec())
}

/// The game replays from its log to the same libraries.
fn replays(driver: &Driver, seats: usize) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    for seat in 0..seats {
        assert_eq!(
            names(&state, Zone::Library(pid(seat))),
            names(driver.state(), Zone::Library(pid(seat)))
        );
        assert_eq!(
            names(&state, Zone::Hand(pid(seat))),
            names(driver.state(), Zone::Hand(pid(seat)))
        );
    }
}

#[rstest]
#[case::the_first_card(2, "cllll", 1)]
#[case::after_two_lands(2, "llcll", 3)]
#[case::deep(3, "lllllcc", 6)]
#[case::at_four_seats(4, "lclcl", 2)]
#[case::no_match_takes_the_library(2, "llll", 4)]
#[case::an_empty_library(2, "", 0)]
fn treasure_hunt_puts_every_card_revealed_into_the_hand(
    #[case] seats: usize,
    #[case] spec: &str,
    #[case] revealed_count: usize,
) -> Result<(), ScriptError> {
    let table = Scenario::new(seats)
        .hand_named(0, ["Treasure Hunt"])
        .battlefield(0, lands(2, island))
        .library(0, library_of(spec));
    let mut driver = Driver::from_scenario(table)?;
    let events = play(&mut driver, "Treasure Hunt")?;
    assert_eq!(
        sorted(names(driver.state(), Zone::Hand(pid(0)))),
        sorted(named(spec, 0..revealed_count))
    );
    assert_eq!(
        top_first(driver.state()),
        named(spec, revealed_count..spec.len()),
        "the cards below the last match stay where they were"
    );
    let reveals = revealed(&events);
    if revealed_count == 0 {
        assert!(reveals.is_empty(), "nothing to reveal is no event");
    } else {
        assert_eq!(reveals.len(), 1);
        assert_eq!(reveals[0].player, pid(0));
        assert_eq!(reveals[0].cards.len(), revealed_count);
    }
    let library = driver.state().zone(Zone::Library(pid(0))).to_vec();
    assert!(
        events
            .iter()
            .flat_map(Event::touched_objects)
            .all(|id| !library.contains(&id)),
        "no event names a card that stayed hidden"
    );
    replays(&driver, seats);
    Ok(())
}

#[rstest]
#[case::first_card_a_land(2, "lccc")]
#[case::two_before_the_land(2, "ccl")]
#[case::deep(3, "cccccl")]
#[case::at_four_seats(4, "cclc")]
#[case::no_land(2, "ccc")]
fn clifftop_lookout_puts_the_first_land_onto_the_battlefield_tapped_and_the_rest_on_the_bottom(
    #[case] seats: usize,
    #[case] spec: &str,
) -> Result<(), ScriptError> {
    let table = Scenario::new(seats)
        .hand_named(0, ["Clifftop Lookout"])
        .battlefield(0, (0..3).map(|_| ready(forest())))
        .library(0, library_of(spec));
    let mut driver = Driver::from_scenario(table)?;
    let before = names(driver.state(), Zone::Battlefield).len();
    let events = play(&mut driver, "Clifftop Lookout")?;
    let land_at = spec.find('l');
    let revealed_count = land_at.map_or(spec.len(), |at| at + 1);
    let reveals = revealed(&events);
    assert_eq!(reveals.len(), 1);
    assert_eq!(reveals[0].cards.len(), revealed_count);
    let state = driver.state();
    let battlefield = names(state, Zone::Battlefield);
    assert_eq!(
        battlefield.len(),
        before + 1 + usize::from(land_at.is_some()),
        "the Lookout and the land found"
    );
    let tapped = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| {
            object.card().name == "Forest" && object.permanent().is_some_and(|p| p.tapped())
        })
        .count();
    assert_eq!(
        tapped,
        3 + usize::from(land_at.is_some()),
        "the three Forests that paid and the land found"
    );
    let library = top_first(state);
    let below = named(spec, revealed_count..spec.len());
    assert_eq!(
        library[..below.len()],
        below,
        "the unrevealed cards stay on top, in order"
    );
    let rest: Vec<String> = named(spec, 0..revealed_count)
        .into_iter()
        .filter(|name| name != "Forest")
        .collect();
    assert_eq!(sorted(library[below.len()..].to_vec()), sorted(rest));
    Ok(())
}
