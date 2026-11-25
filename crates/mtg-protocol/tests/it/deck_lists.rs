//! Goal: each viewer looks through its own deck and every public list whole, grouped by card type with copies counted, and of a hidden deck only its commanders and companion; each list carries the tokens its listed cards make, read from the card definitions, so a hidden deck's tokens show only where its commander makes them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::GameSetup;
use mtg_protocol::{ListGroup, SeatList, deck_lists};
use mtg_testkit::corpus::card;
use mtg_testkit::fuzz_setup;
use rstest::rstest;

fn cards(names: &[&str]) -> Vec<Arc<CardDefinition>> {
    names.iter().map(|name| card(name).unwrap()).collect()
}

/// A table of two seats, each with its commanders and library of corpus cards.
fn table(decks: [(&[&str], &[&str]); 2]) -> GameSetup {
    let mut setup = fuzz_setup(2, 1);
    for (seat, (commanders, library)) in setup.players.iter_mut().zip(decks) {
        seat.commanders = cards(commanders);
        seat.library = cards(library);
        seat.hand.clear();
        seat.graveyard.clear();
        seat.battlefield.clear();
    }
    setup
}

fn list(lists: &[SeatList], seat: usize) -> Option<&SeatList> {
    lists
        .iter()
        .find(|list| list.seat == PlayerId::from_index(seat))
}

fn token_names(list: &SeatList) -> Vec<&str> {
    list.tokens
        .iter()
        .map(|token| token.name.as_str())
        .collect()
}

const GOBLINS: &[&str] = &[
    "Dragon Fodder",
    "Hordeling Outburst",
    "Mountain",
    "Mountain",
];
const RICHES: &[&str] = &["Smothering Tithe", "Tireless Tracker", "Forest"];
const THOPTERS: &[&str] = &["Pia Nalaar", "Mountain"];
const SOLDIERS: &[&str] = &["Elspeth Tirel", "Plains"];

#[rstest]
#[case::goblins_one_token_however_many_makers(&["Krenko, Mob Boss"], GOBLINS, &["Goblin"])]
#[case::treasure_and_clue(&[], RICHES, &["Treasure", "Clue"])]
#[case::thopters(&[], THOPTERS, &["Thopter"])]
#[case::soldiers(&[], SOLDIERS, &["Soldier"])]
#[case::no_maker(&[], &["Mountain", "Forest"], &[])]
fn a_deck_lists_the_tokens_its_cards_make(
    #[case] commanders: &[&str],
    #[case] library: &[&str],
    #[case] expected: &[&str],
) {
    let setup = table([(commanders, library), (&[], &["Plains"])]);
    let lists = deck_lists(&setup, PlayerId::from_index(0), &[]);
    let own = list(&lists, 0).unwrap();
    assert!(own.whole);
    let mut names = token_names(own);
    names.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(names, expected);
}

#[test]
fn a_token_is_told_by_the_facts_its_picture_is_found_by() {
    let setup = table([(&["Krenko, Mob Boss"], &[]), (&[], &[])]);
    let lists = deck_lists(&setup, PlayerId::from_index(0), &[]);
    let goblin = &list(&lists, 0).unwrap().tokens[0];
    assert_eq!(goblin.name, "Goblin");
    assert_eq!(goblin.colors, vec![Color::Red]);
    assert_eq!(goblin.types, vec![CardType::Creature]);
    assert_eq!((goblin.power, goblin.toughness), (Some(1), Some(1)));
}

#[test]
fn a_whole_list_is_grouped_by_type_with_copies_counted() {
    let setup = table([
        (
            &["Krenko, Mob Boss"],
            &[
                "Tireless Tracker",
                "Smothering Tithe",
                "Dragon Fodder",
                "Mountain",
                "Mountain",
            ],
        ),
        (&[], &[]),
    ]);
    let lists = deck_lists(&setup, PlayerId::from_index(0), &[]);
    let lines: Vec<(ListGroup, &str, u32)> = list(&lists, 0)
        .unwrap()
        .cards
        .iter()
        .map(|line| (line.group, line.name.as_str(), line.count))
        .collect();
    assert_eq!(
        lines,
        vec![
            (ListGroup::Commander, "Krenko, Mob Boss", 1),
            (ListGroup::Creature, "Tireless Tracker", 1),
            (ListGroup::Sorcery, "Dragon Fodder", 1),
            (ListGroup::Enchantment, "Smothering Tithe", 1),
            (ListGroup::Land, "Mountain", 2),
        ]
    );
}

/// Seat 0 leads with Pia Nalaar (Thopters) and hides a library of Treasure and Clue makers.
fn hidden_table() -> GameSetup {
    table([(&["Pia Nalaar"], RICHES), (&["Krenko, Mob Boss"], GOBLINS)])
}

#[rstest]
#[case::the_opponent(1)]
#[case::a_spectator(2)]
fn a_hidden_deck_shows_its_commander_and_the_commanders_tokens_alone(#[case] viewer: usize) {
    let lists = deck_lists(&hidden_table(), PlayerId::from_index(viewer), &[]);
    let theirs = list(&lists, 0).unwrap();
    assert!(!theirs.whole);
    let names: Vec<&str> = theirs.cards.iter().map(|line| line.name.as_str()).collect();
    assert_eq!(names, vec!["Pia Nalaar"]);
    assert_eq!(token_names(theirs), vec!["Thopter"]);
    let text = serde_json::to_string(&lists).unwrap();
    for hidden in ["Smothering Tithe", "Tireless Tracker", "Treasure", "Clue"] {
        assert!(!text.contains(hidden), "viewer {viewer} is told {hidden}");
    }
}

#[rstest]
#[case::the_opponent(1)]
#[case::a_spectator(2)]
fn a_public_deck_shows_whole_with_every_token(#[case] viewer: usize) {
    let lists = deck_lists(
        &hidden_table(),
        PlayerId::from_index(viewer),
        &[PlayerId::from_index(0)],
    );
    let theirs = list(&lists, 0).unwrap();
    assert!(theirs.whole);
    assert_eq!(theirs.cards.len(), 1 + RICHES.len());
    let mut names = token_names(theirs);
    names.sort_unstable();
    assert_eq!(names, vec!["Clue", "Thopter", "Treasure"]);
}

#[test]
fn a_seat_with_nothing_to_show_is_left_out() {
    let setup = table([(&[], &["Mountain"]), (&[], &["Plains"])]);
    let seats: Vec<PlayerId> = deck_lists(&setup, PlayerId::from_index(0), &[])
        .iter()
        .map(|list| list.seat)
        .collect();
    assert_eq!(seats, vec![PlayerId::from_index(0)]);
}
