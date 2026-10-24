//! Goal: a miracle that Molecule Man gives to the nonland cards of a hand discloses what a printed miracle does and nothing more, at two, three and four seats: the card drawn first in a turn is named to every seat by its trigger on the stack and nothing else of the hand is, while a card drawn second (no trigger) stays unnamed in every other seat's events, table and objects.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::GameState;
use mtg_engine::zones::Zone;
use mtg_protocol::project_events;
use mtg_testkit::cards::{forest, island, mountain, study, wastes};
use mtg_testkit::corpus::card as corpus_card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::hidden_info::{assert_clean, hidden_in};

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Seat 0 has Molecule Man (miracle {0} for each nonland card of its hand) and two lands in play, holds a Study and two
/// cards that stay hidden, and draws the cards `library` names, then Forests; every other seat has a hand of its own.
fn table(seats: usize, library: &[&str]) -> Scenario {
    let mut cards: Vec<_> = library
        .iter()
        .map(|name| match *name {
            "Wastes" => wastes(),
            other => corpus_card(other).unwrap(),
        })
        .collect();
    cards.extend(vec![forest(); 6]);
    let mut scenario = Scenario::new(seats)
        .library(0, cards)
        .battlefield(
            0,
            [
                ready(corpus_card("Molecule Man").unwrap()),
                ready(island()),
                ready(mountain()),
            ],
        )
        .hand(
            0,
            [study("Study")]
                .into_iter()
                .chain(["Counterspell", "Wrath of God"].map(|name| corpus_card(name).unwrap())),
        );
    let others = [
        ["Serra Angel", "Doom Blade"],
        ["Fireball", "Opt"],
        ["Brainstorm", "Llanowar Elves"],
    ];
    for other in 1..seats {
        scenario = scenario
            .hand_named(other, others[other - 1])
            .library(other, vec![forest(); 6]);
    }
    scenario
}

/// Seat 0 casts Study and its draws are played out: the state before it, and the driver after.
fn drawn(seats: usize, library: &[&str]) -> (GameState, Driver) {
    let scenario = table(seats, library);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Study").unwrap();
    for _ in 0..seats {
        driver.pass().unwrap();
    }
    (before, driver)
}

/// The card called `name` in seat 0's hand.
fn in_hand(state: &GameState, name: &str) -> ObjectId {
    state
        .zone(Zone::Hand(seat(0)))
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .expect("the card is in the hand")
}

#[rstest]
fn the_trigger_of_a_granted_miracle_names_its_card_to_every_seat_and_the_rest_of_the_hand_is_not(
    #[values(2, 3, 4)] seats: usize,
) {
    let (before, driver) = drawn(seats, &["Giant Growth", "Lightning Bolt"]);
    let after = driver.state().clone();
    assert_eq!(
        after.zone(Zone::Stack).len(),
        1,
        "the miracle trigger is waiting"
    );
    let card = in_hand(&after, "Giant Growth");
    let events = driver.engine().events().to_vec();
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        let mut hidden = hidden_in(&after, *viewer);
        // The trigger names the card it reveals; nothing else of the hand is named.
        hidden.remove(&card);
        assert_eq!(
            hidden.values().any(|name| name == "Wrath of God"),
            viewer.index() != 0,
            "only the owner sees the rest of the hand"
        );
        assert_clean(batch, &hidden);
        assert!(
            serde_json::to_string(&batch.objects)
                .unwrap()
                .contains("Giant Growth"),
            "seat {} sees the trigger name the card",
            viewer.index()
        );
    }
}

#[rstest]
fn a_card_with_a_granted_miracle_that_is_not_the_first_draw_stays_unnamed(
    #[values(2, 3, 4)] seats: usize,
) {
    // A Wastes is drawn first, so the nonland card drawn second triggers nothing.
    let (before, driver) = drawn(seats, &["Wastes", "Giant Growth"]);
    let after = driver.state().clone();
    assert!(after.zone(Zone::Stack).is_empty(), "no trigger");
    let events = driver.engine().events().to_vec();
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        let hidden = hidden_in(&after, *viewer);
        assert_eq!(
            hidden.values().any(|name| name == "Giant Growth"),
            viewer.index() != 0,
            "the card is in the owner's hand"
        );
        assert_clean(batch, &hidden);
    }
}
