//! Goal: a land that chooses a color other than its own as it enters (the Thriving lands, the Gates) asks its controller for one of the other four colors, enters tapped, remembers the choice and offers its own color and the chosen one and no other; a land that enters tapped with two charge counters (the Vivid lands) offers its own color, and any color while a counter is left to remove, taking the counter when it is used; whole bot games with these lands at tables of two, three and four seats finish and keep every invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use common::seeds::for_each_seed;
use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_core::mana::ManaType;
use mtg_engine::Event;
use mtg_engine::GameSetup;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Normal;
use mtg_engine::object::Chosen;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, PlayOptions, Scenario, play_game_with, ready, with_counters};
use rstest::rstest;

/// Seat 0 holds the cards `hand`; every seat has a library to draw from.
fn table(seats: usize, hand: &[&str], mine: Vec<mtg_engine::setup::PresetPermanent>) -> Driver {
    let mut scenario = Scenario::new(seats)
        .hand(0, hand.iter().map(|name| card(name).unwrap()))
        .battlefield(0, mine);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    Driver::from_scenario(scenario).unwrap()
}

fn charge() -> CounterKind {
    CounterKind::Charge
}

fn on_the_battlefield(driver: &Driver, name: &str) -> mtg_engine::object::PermanentState {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .clone()
}

/// The types of mana the named permanent offers to the player holding priority.
fn offered(driver: &Driver, name: &str) -> BTreeSet<ManaType> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    options
        .mana_abilities
        .iter()
        .filter(|m| name_of(driver.state(), m.source.object) == name)
        .map(|m| m.produces)
        .collect()
}

/// The lands that choose a color other than the one they make, with the color they make.
const CHOOSERS: [(&str, Color); 10] = [
    ("Thriving Bluff", Color::Red),
    ("Thriving Grove", Color::Green),
    ("Thriving Heath", Color::White),
    ("Thriving Isle", Color::Blue),
    ("Thriving Moor", Color::Black),
    ("Citadel Gate", Color::White),
    ("Cliffgate", Color::Red),
    ("Sea Gate", Color::Blue),
    ("Manor Gate", Color::Green),
    ("Black Dragon Gate", Color::Black),
];

/// Plays `land` on the first turn and answers the color question with the `pick`th color offered.
fn entered(seats: usize, land: &str, own: Color, pick: usize) -> (Driver, Color) {
    let mut driver = table(seats, &[land], Vec::new());
    driver.play_land(land).unwrap();
    let decision = driver.decision().unwrap().clone();
    let DecisionKind::ChooseOption(options) = &decision.kind else {
        panic!(
            "a color is chosen as {land} enters, not {:?}",
            decision.kind
        );
    };
    assert_eq!(decision.player, pid(0));
    assert_eq!(
        options.text,
        format!(
            "Choose a color other than {}.",
            format!("{own:?}").to_lowercase()
        )
    );
    let others: Vec<Color> = Color::ALL.into_iter().filter(|c| *c != own).collect();
    assert_eq!(
        options.options,
        others
            .iter()
            .copied()
            .map(ChoiceOption::Color)
            .collect::<Vec<_>>()
    );
    driver
        .answer(Answer::ChooseOption(OptionChoice(pick)))
        .unwrap();
    (driver, others[pick])
}

#[rstest]
fn a_land_offers_the_other_four_colors_and_keeps_the_one_chosen_and_enters_tapped(
    #[values(
        ("Thriving Bluff", Color::Red),
        ("Thriving Isle", Color::Blue),
        ("Thriving Heath", Color::White),
        ("Citadel Gate", Color::White),
        ("Black Dragon Gate", Color::Black),
        ("Manor Gate", Color::Green)
    )]
    land: (&str, Color),
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] pick: usize,
) {
    let (driver, chosen) = entered(seats, land.0, land.1, pick);
    let state = on_the_battlefield(&driver, land.0);
    assert_eq!(state.chosen().cloned(), Some(Chosen::Color(chosen)));
    assert!(state.tapped(), "{} enters tapped", land.0);
}

#[rstest]
fn a_land_offers_its_own_color_and_the_chosen_one_when_it_untaps_and_no_other(
    #[values(0, 1, 2)] land: usize,
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] pick: usize,
) {
    let (name, own) = [CHOOSERS[0], CHOOSERS[7], CHOOSERS[8]][land];
    let (mut driver, chosen) = entered(seats, name, own, pick);
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Main1).unwrap();
    assert!(!on_the_battlefield(&driver, name).tapped());
    let (own, chosen) = (ManaType::from(own), ManaType::from(chosen));
    assert_eq!(offered(&driver, name), BTreeSet::from([own, chosen]));
    // A color that is neither is not on offer.
    let neither = [
        ManaType::White,
        ManaType::Blue,
        ManaType::Black,
        ManaType::Red,
        ManaType::Green,
        ManaType::Colorless,
    ]
    .into_iter()
    .find(|mana| *mana != own && *mana != chosen)
    .unwrap();
    assert!(driver.tap_for_mana_of(name, neither).is_err());
    let events = driver.tap_for_mana_of(name, chosen).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ManaAdded { mana, .. } if *mana == chosen
    )));
    assert!(on_the_battlefield(&driver, name).tapped());
}

const VIVID: [(&str, ManaType); 5] = [
    ("Vivid Crag", ManaType::Red),
    ("Vivid Creek", ManaType::Blue),
    ("Vivid Grove", ManaType::Green),
    ("Vivid Marsh", ManaType::Black),
    ("Vivid Meadow", ManaType::White),
];

#[rstest]
fn a_vivid_land_enters_tapped_with_two_charge_counters(
    #[values(0, 1, 2, 3, 4)] land: usize,
    #[values(2, 3, 4)] seats: usize,
) {
    let name = VIVID[land].0;
    let mut driver = table(seats, &[name], Vec::new());
    driver.play_land(name).unwrap();
    let state = on_the_battlefield(&driver, name);
    assert!(state.tapped());
    assert_eq!(state.counters_of(&charge()), 2);
}

#[rstest]
#[case::two_counters(2, true)]
#[case::one_counter(1, true)]
#[case::no_counter(0, false)]
fn a_vivid_land_offers_any_color_while_a_counter_is_left_and_takes_it(
    #[case] counters: u32,
    #[case] any_color: bool,
    #[values(0, 1, 2, 3, 4)] land: usize,
    #[values(2, 3, 4)] seats: usize,
) {
    let (name, own) = VIVID[land];
    let permanent = with_counters(ready(card(name).unwrap()), charge(), counters);
    let mut driver = table(seats, &[], vec![permanent]);
    let expected: BTreeSet<ManaType> = if any_color {
        BTreeSet::from([
            ManaType::White,
            ManaType::Blue,
            ManaType::Black,
            ManaType::Red,
            ManaType::Green,
        ])
    } else {
        BTreeSet::from([own])
    };
    assert_eq!(offered(&driver, name), expected);
    let other = [
        ManaType::White,
        ManaType::Blue,
        ManaType::Black,
        ManaType::Red,
    ]
    .into_iter()
    .find(|mana| *mana != own)
    .unwrap();
    if any_color {
        driver.tap_for_mana_of(name, other).unwrap();
        assert_eq!(
            on_the_battlefield(&driver, name).counters_of(&charge()),
            counters - 1
        );
    } else {
        assert!(driver.tap_for_mana_of(name, other).is_err());
        driver.tap_for_mana_of(name, own).unwrap();
        assert_eq!(on_the_battlefield(&driver, name).counters_of(&charge()), 0);
    }
    assert!(on_the_battlefield(&driver, name).tapped());
}

fn definition(name: &str) -> Arc<CardDefinition> {
    match name {
        "Forest" => forest(),
        "Island" => island(),
        "Mountain" => mountain(),
        "Plains" => plains(),
        "Swamp" => swamp(),
        other => card(other).unwrap(),
    }
}

fn deck() -> Vec<Arc<CardDefinition>> {
    let mut deck: Vec<(&str, usize)> = vec![
        ("Forest", 5),
        ("Island", 5),
        ("Mountain", 5),
        ("Plains", 5),
        ("Swamp", 5),
        ("Grizzly Bears", 4),
        ("Serra Angel", 4),
        ("Giant Growth", 4),
        ("Lightning Bolt", 4),
        ("Counterspell", 4),
    ];
    deck.extend(CHOOSERS.iter().map(|(name, _)| (*name, 1)));
    deck.extend(VIVID.iter().map(|(name, _)| (*name, 1)));
    deck.into_iter()
        .flat_map(|(name, copies)| (0..copies).map(move |_| definition(name)))
        .collect()
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_these_lands_finish_and_keep_every_invariant(#[case] players: usize) {
    let choices = Mutex::new(0u64);
    let games: u64 = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    for_each_seed(0..games, |seed| {
        let decks = (0..players).map(|_| deck()).collect();
        let setup = GameSetup::new(Arc::new(Normal), seed, decks);
        let outcome = play_game_with(setup, seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"));
        let made = outcome
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Event::Entered {
                        chosen: Some(Chosen::Color(_)),
                        ..
                    }
                )
            })
            .count();
        *choices.lock().unwrap() += made as u64;
    });
    assert!(
        choices.into_inner().unwrap() > 0,
        "the games should choose a color as a land enters"
    );
}
