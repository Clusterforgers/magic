//! Goal: Blood Moon, Magus of the Moon and Dryad of the Ilysian Grove, read from their Oracle text, play Verified at tables of two and four seats as the rules say: nonbasic lands of every player are Mountains that tap for {R} alone and keep no trigger or ability of their text, basic lands stay as printed, the Dryad makes the lands of its controller every basic land type and allows a second land play, the later of the two decides a land under both, and bot games of decks with the three cards finish and keep every invariant at two, three and four seats. (t19f)
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::common::seeds::{for_each_seed, fuzz_games};

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::support::Support;
use mtg_core::card::Subtype;
use mtg_core::mana::ManaType::{self, Black, Blue, Colorless, Green, Red, White};
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Normal;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameSetup, replay};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::{card, corpus};
use mtg_testkit::{Driver, Scenario, play_game, ready};
use rstest::rstest;

const BLOOD_MOON: &str = "Blood Moon";
const MAGUS: &str = "Magus of the Moon";
const DRYAD: &str = "Dryad of the Ilysian Grove";
const BASIC: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];

/// Seat 0 has `permanents` in this order, seat 1 has `opposing`, and seat 0 has `hand` at the first main step of turn 1.
fn table(seats: usize, permanents: &[&str], opposing: &[&str], hand: &[&str]) -> Driver {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, permanents.iter().map(|name| ready(card(name).unwrap())))
        .battlefield(1, opposing.iter().map(|name| ready(card(name).unwrap())))
        .hand(0, hand.iter().map(|name| card(name).unwrap()));
    Driver::from_scenario(scenario).unwrap()
}

/// The types of mana the named permanent offers to the player holding priority, one entry for each way, sorted.
fn offered(driver: &Driver, name: &str) -> Vec<ManaType> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    let mut found: Vec<ManaType> = options
        .mana_abilities
        .iter()
        .filter(|m| name_of(driver.state(), m.source.object) == name)
        .map(|m| m.produces)
        .collect();
    found.sort();
    found
}

/// The basic land types the named permanent has now, in the order of the rules.
fn land_types(driver: &Driver, name: &str) -> Vec<&'static str> {
    let id = driver.id_of(name).unwrap();
    let object = driver.state().object(id).unwrap();
    let chars = characteristics(driver.state(), id).unwrap();
    BASIC
        .into_iter()
        .filter(|kind| chars.has_subtype(&object.card().subtypes, &Subtype((*kind).to_owned())))
        .collect()
}

#[rstest]
#[case::blood_moon(BLOOD_MOON)]
#[case::magus_of_the_moon(MAGUS)]
#[case::dryad_of_the_ilysian_grove(DRYAD)]
fn the_cards_play_verified(#[case] name: &str) {
    let record = corpus().unwrap().support(name).unwrap();
    assert_eq!(record.support, Support::Verified, "{name}");
}

#[rstest]
#[case::blinkmoth_nexus_loses_its_colorless(&[BLOOD_MOON, "Blinkmoth Nexus"], "Blinkmoth Nexus", &[Red])]
#[case::cradle_loses_its_count(&[BLOOD_MOON, "Gaea's Cradle"], "Gaea's Cradle", &[Red])]
#[case::city_of_brass_loses_its_colors(&[BLOOD_MOON, "City of Brass"], "City of Brass", &[Red])]
#[case::cataracts_loses_its_colors(&[MAGUS, "Cascading Cataracts"], "Cascading Cataracts", &[Red])]
#[case::a_basic_land_stays_as_printed(&[BLOOD_MOON, "Forest"], "Forest", &[Green])]
#[case::a_basic_land_under_magus(&[MAGUS, "Forest"], "Forest", &[Green])]
#[case::nexus_without_the_moon(&["Blinkmoth Nexus"], "Blinkmoth Nexus", &[Colorless])]
#[case::city_without_the_moon(&["City of Brass"], "City of Brass", &[White, Blue, Black, Red, Green])]
fn a_land_offers_the_mana_of_the_types_the_moon_leaves_it(
    #[values(2, 4)] seats: usize,
    #[case] permanents: &[&str],
    #[case] land: &str,
    #[case] mana: &[ManaType],
) {
    let driver = table(seats, permanents, &[], &[]);
    let mut wanted = mana.to_vec();
    wanted.sort();
    assert_eq!(offered(&driver, land), wanted, "{permanents:?}");
}

#[rstest]
#[case::the_moon(&[BLOOD_MOON, "Blinkmoth Nexus"], &[], "Blinkmoth Nexus", &["Mountain"])]
#[case::magus(&[MAGUS, "City of Brass"], &[], "City of Brass", &["Mountain"])]
#[case::a_basic_land(&[BLOOD_MOON, "Forest"], &[], "Forest", &["Forest"])]
#[case::an_opponents_nonbasic_land(&[BLOOD_MOON], &["City of Brass"], "City of Brass", &["Mountain"])]
#[case::an_opponents_basic_land(&[MAGUS], &["Forest"], "Forest", &["Forest"])]
#[case::dryad_on_a_basic_land(&[DRYAD, "Forest"], &[], "Forest", &BASIC)]
#[case::dryad_on_a_nonbasic_land(&[DRYAD, "Blinkmoth Nexus"], &[], "Blinkmoth Nexus", &BASIC)]
#[case::dryad_leaves_an_opponents_land(&[DRYAD], &["City of Brass"], "City of Brass", &[])]
#[case::the_dryad_first_and_the_moon_later(&[DRYAD, BLOOD_MOON, "Blinkmoth Nexus"], &[], "Blinkmoth Nexus", &["Mountain"])]
#[case::the_moon_first_and_the_dryad_later(&[BLOOD_MOON, DRYAD, "Blinkmoth Nexus"], &[], "Blinkmoth Nexus", &BASIC)]
#[case::the_basic_land_under_both(&[DRYAD, BLOOD_MOON, "Forest"], &[], "Forest", &BASIC)]
fn the_land_types_of_a_land_follow_the_effects_in_timestamp_order(
    #[values(2, 4)] seats: usize,
    #[case] permanents: &[&str],
    #[case] opposing: &[&str],
    #[case] land: &str,
    #[case] types: &[&str],
) {
    let driver = table(seats, permanents, opposing, &[]);
    assert_eq!(land_types(&driver, land), types, "{permanents:?}");
}

#[rstest]
#[case::the_dryad_first(&[DRYAD, BLOOD_MOON, "Command Tower"], "Command Tower", &[Red])]
#[case::the_moon_first(&[BLOOD_MOON, DRYAD, "Command Tower"], "Command Tower", &[White, Blue, Black, Red, Green])]
#[case::the_dryad_alone(&[DRYAD, "Command Tower"], "Command Tower", &[White, Blue, Black, Red, Green])]
#[case::the_dryad_on_a_basic_land(&[DRYAD, "Forest"], "Forest", &[White, Blue, Black, Red, Green])]
#[case::the_dryad_on_a_nexus_keeps_its_own(&[DRYAD, "Blinkmoth Nexus"], "Blinkmoth Nexus", &[Colorless, White, Blue, Black, Red, Green])]
fn a_land_offers_the_mana_of_each_type_it_ends_with(
    #[values(2, 4)] seats: usize,
    #[case] permanents: &[&str],
    #[case] land: &str,
    #[case] mana: &[ManaType],
) {
    let driver = table(seats, permanents, &[], &[]);
    let mut wanted = mana.to_vec();
    wanted.sort();
    assert_eq!(offered(&driver, land), wanted, "{permanents:?}");
}

#[rstest]
#[case::without_the_moon(&[], 1)]
#[case::under_blood_moon(&[BLOOD_MOON], 0)]
#[case::under_magus(&[MAGUS], 0)]
fn a_nonbasic_land_under_the_moon_has_lost_its_triggers(
    #[values(2, 4)] seats: usize,
    #[case] effects: &[&str],
    #[case] triggered: usize,
) {
    let permanents: Vec<&str> = std::iter::once("City of Brass")
        .chain(effects.iter().copied())
        .collect();
    let mut driver = table(seats, &permanents, &[], &[]);
    driver.tap_for_mana_of("City of Brass", Red).unwrap();
    assert_eq!(driver.state().stack().len(), triggered, "{effects:?}");
}

#[rstest]
#[case::with_the_dryad(&[DRYAD], true)]
#[case::without_it(&[], false)]
fn the_dryad_allows_a_second_land_play(
    #[values(2, 4)] seats: usize,
    #[case] permanents: &[&str],
    #[case] second: bool,
) {
    let mut driver = table(seats, permanents, &[], &["Forest", "Forest", "Forest"]);
    driver.play_land("Forest").unwrap();
    assert_eq!(driver.play_land("Forest").is_ok(), second, "{permanents:?}");
    if second {
        assert!(driver.play_land("Forest").is_err(), "no third land play");
    }
}

/// Lands, the three cards and spells to cast around them: sixty cards with no more than four of a card.
fn deck() -> Vec<Arc<CardDefinition>> {
    let mut deck: Vec<Arc<CardDefinition>> = Vec::new();
    for _ in 0..6 {
        deck.extend([forest(), island(), mountain(), plains(), swamp()]);
    }
    deck.extend(std::iter::repeat_n(mountain(), 3));
    for (name, copies) in [
        (BLOOD_MOON, 3),
        (MAGUS, 3),
        (DRYAD, 3),
        ("Blinkmoth Nexus", 2),
        ("City of Brass", 1),
        ("Gaea's Cradle", 1),
        ("Cascading Cataracts", 1),
        ("Command Tower", 1),
        ("Grizzly Bears", 4),
        ("Lightning Bolt", 4),
        ("Giant Growth", 4),
    ] {
        deck.extend(std::iter::repeat_n(card(name).unwrap(), copies));
    }
    deck
}

#[test]
fn the_deck_is_sixty_cards_with_the_three_cards_in_it() {
    let deck = deck();
    assert_eq!(deck.len(), 60);
    for name in [BLOOD_MOON, MAGUS, DRYAD] {
        assert!(deck.iter().any(|card| card.name == name), "{name}");
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_the_three_cards_finish_and_keep_every_invariant(#[case] players: usize) {
    let entered: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());
    for_each_seed(0..fuzz_games(20), |seed| {
        let decks = (0..players).map(|_| deck()).collect();
        let setup = GameSetup::new(Arc::new(Normal), seed, decks);
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        let mut entered = entered.lock().unwrap();
        for event in &outcome.events {
            if let Event::SpellResolved { object } = event
                && let Some(object) = end.object(*object)
            {
                *entered.entry(object.card().name.clone()).or_default() += 1;
            }
        }
    });
    let entered = entered.into_inner().unwrap();
    for name in [BLOOD_MOON, MAGUS, DRYAD] {
        assert!(
            entered.get(name).copied().unwrap_or(0) > 0,
            "{name} resolved in no game of {} at {players} seats: {entered:?}",
            fuzz_games(20)
        );
    }
}

#[test]
fn a_seeded_game_with_the_three_cards_is_the_same_game_every_time() {
    let play = |seed: u64| {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(), deck()]);
        play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"))
    };
    let (first, second) = (play(5), play(5));
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
