//! Goal: a power and toughness bonus counted "for each" (Borderland Behemoth: +4/+4 for each other Giant you control) is as big as the Giants its controller has, whichever seat has them and at tables of two, three and four seats, and whole bot games at two, three and four seats with decks of the corpus cards that count (cards, life, mana and tokens "for each", a pump "for each Swamp") finish, keep every invariant and cast every one of those cards.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::characteristics::characteristics;
use mtg_engine::{Event, GameState, replay};
use mtg_testkit::count_cards::COUNT_CARDS;
use mtg_testkit::{Scenario, count_fuzz_setup, play_game, ready};
use rstest::rstest;

fn giant() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Giant")
            .with_type(CardType::Creature)
            .with_subtype("Giant")
            .with_power_toughness(2, 2),
    )
}

/// A table of `seats` where seat 0 has the Behemoth and `own` Giants, and each other seat has `others` Giants.
fn table(seats: usize, own: usize, others: usize) -> GameState {
    let mut scenario = Scenario::new(seats);
    let mut mine = vec![ready(
        mtg_testkit::corpus::card("Borderland Behemoth").unwrap(),
    )];
    mine.extend((0..own).map(|_| ready(giant())));
    scenario = scenario.battlefield(0, mine);
    for seat in 1..seats {
        let theirs: Vec<_> = (0..others).map(|_| ready(giant())).collect();
        scenario = scenario.battlefield(seat, theirs);
    }
    scenario.build().unwrap().state().clone()
}

fn behemoth(game: &GameState) -> (Option<i32>, Option<i32>) {
    let id = game
        .objects()
        .find(|object| object.card().name == "Borderland Behemoth")
        .unwrap()
        .id();
    let shown = characteristics(game, id).unwrap();
    (shown.power, shown.toughness)
}

#[rstest]
fn the_behemoth_grows_by_four_for_each_other_giant_its_controller_has(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] own: usize,
    #[values(0, 2)] others: usize,
) {
    let (power, toughness) = behemoth(&table(seats, own, others));
    // The printed 4/4 plus 4/+4 for each Giant of its own side; the Giants of the other seats do not count.
    let bonus = 4 * i32::try_from(own).unwrap();
    assert_eq!((power, toughness), (Some(4 + bonus), Some(4 + bonus)));
}

/// The names of the cards cast in a finished game.
fn cast_cards(setup: &mtg_engine::GameSetup, events: &[Event]) -> BTreeSet<String> {
    let state = replay(setup, events).unwrap();
    events
        .iter()
        .filter_map(|event| match event {
            Event::SpellCast { object, .. } => state
                .object(*object)
                .map(|object| object.card().name.clone()),
            _ => None,
        })
        .collect()
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_counting_games_finish_keep_every_invariant_and_cast_every_card(#[case] players: usize) {
    let games: u64 = fuzz_games(30);
    let mut seen = BTreeSet::new();
    for seed in 0..games {
        let setup = count_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.extend(cast_cards(&setup, &outcome.events));
    }
    for &(name, _) in COUNT_CARDS {
        assert!(
            seen.contains(name),
            "{name} never cast in {games} games of {players} seats: {seen:?}"
        );
    }
}
