//! Goal: the storage-counter lands read from Oracle text play as the rules say at tables of two, three and four seats: removing X counters pays X mana of the land's types with its {1} paid by another land, a color it never makes or more mana than the counters hold is out of reach, the counters really leave the land, the Crucible's mana pays for Dragons only, and whole bot games with these lands keep every invariant and use the ability.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::{for_each_seed, fuzz_games};

use std::sync::{Arc, Mutex};

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::GameSetup;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::Event;
use mtg_engine::format::Normal;
use mtg_testkit::cards::{forest, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, PlayOptions, Scenario, play_game_with, ready, with_counters};
use rstest::rstest;

fn storage() -> CounterKind {
    CounterKind::Other("storage".into())
}

fn spell(cost: &str, subtype: Option<&str>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Trick")
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1)
        .with_mana_cost(ManaCost::parse(cost).unwrap());
    if let Some(subtype) = subtype {
        card = card.with_subtype(subtype);
    }
    Arc::new(card)
}

/// Every seat has a library to draw from.
fn libraries(mut scenario: Scenario, seats: usize) -> Scenario {
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    scenario
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

/// The counters taken from permanents in the events, as a total.
fn removed(events: &[Event]) -> i32 {
    events
        .iter()
        .filter_map(|event| match event {
            Event::CountersChanged(changed) if changed.delta < 0 => Some(-changed.delta),
            _ => None,
        })
        .sum()
}

#[rstest]
#[case::two_counters_make_red_and_green("Fungal Reaches", 2, 1, "{R}{G}", None, 2)]
#[case::two_counters_make_two_of_one_type("Fungal Reaches", 2, 1, "{R}{R}", None, 2)]
#[case::three_counters_make_three("Molten Slagheap", 3, 1, "{B}{R}{R}", None, 3)]
#[case::one_counter_is_not_enough("Fungal Reaches", 1, 1, "{R}{G}", None, 0)]
#[case::a_type_the_land_never_makes("Fungal Reaches", 2, 1, "{W}{G}", None, 0)]
#[case::the_ability_costs_a_mana_to_use("Calciform Pools", 2, 0, "{W}{U}", None, 0)]
#[case::the_other_pair("Dreadship Reef", 2, 1, "{U}{B}", None, 2)]
#[case::the_last_pair("Saltcrusted Steppe", 2, 1, "{G}{W}", None, 2)]
#[case::the_crucible_pays_for_a_dragon(
    "Crucible of the Spirit Dragon",
    2,
    0,
    "{W}{B}",
    Some("Dragon"),
    2
)]
#[case::the_crucible_pays_for_nothing_else(
    "Crucible of the Spirit Dragon",
    2,
    0,
    "{W}{B}",
    Some("Elf"),
    0
)]
#[case::the_crucible_pays_for_no_plain_spell(
    "Crucible of the Spirit Dragon",
    2,
    0,
    "{W}{B}",
    None,
    0
)]
fn removing_x_counters_pays_x_mana_of_the_land_and_leaves_the_land_without_them(
    #[values(2, 3, 4)] seats: usize,
    #[case] land: &str,
    #[case] counters: u32,
    #[case] forests: usize,
    #[case] cost: &str,
    #[case] subtype: Option<&str>,
    #[case] taken: i32,
) {
    let mut board = vec![with_counters(
        ready(card(land).unwrap()),
        storage(),
        counters,
    )];
    board.extend((0..forests).map(|_| ready(forest())));
    let scenario = Scenario::new(seats)
        .hand(0, [spell(cost, subtype)])
        .battlefield(0, board);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    assert_eq!(castable(&driver), taken > 0, "{land} for {cost}");
    if taken > 0 {
        let events = driver.cast("Trick").unwrap();
        assert_eq!(removed(&events), taken, "{events:?}");
    }
}

/// Sixty cards: the six lands beside basic lands and spells to pay for.
fn deck() -> Vec<Arc<CardDefinition>> {
    [
        ("Fungal Reaches", 2),
        ("Molten Slagheap", 2),
        ("Crucible of the Spirit Dragon", 1),
        ("Lotus Petal", 4),
        ("Dockside Extortionist", 3),
        ("Llanowar Elves", 4),
        ("Grizzly Bears", 4),
        ("Sol Ring", 4),
        ("Giant Growth", 4),
        ("Lightning Bolt", 4),
        ("Counterspell", 3),
    ]
    .into_iter()
    .flat_map(|(name, copies)| (0..copies).map(move |_| card(name).unwrap()))
    .chain((0..16).map(|_| forest()))
    .chain((0..9).map(|_| mountain()))
    .collect()
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_storage_lands_finish_and_keep_every_invariant(#[case] players: usize) {
    let used = Mutex::new((0_u64, 0_u64));
    for_each_seed(0..fuzz_games(12), |seed| {
        let decks = (0..players).map(|_| deck()).collect();
        let setup = GameSetup::new(Arc::new(Normal), seed, decks);
        let options = PlayOptions {
            partial_payments: seed % 2 == 0,
            ..PlayOptions::default()
        };
        let outcome =
            play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
        let mut used = used.lock().unwrap();
        for event in &outcome.events {
            match event {
                Event::CountersChanged(changed) if changed.delta > 0 => used.0 += 1,
                Event::CountersChanged(changed) if changed.delta < 0 => used.1 += 1,
                _ => {}
            }
        }
    });
    let (stored, spent) = used.into_inner().unwrap();
    assert!(
        stored > 0,
        "the games should store counters: {stored} stored, {spent} spent"
    );
}
