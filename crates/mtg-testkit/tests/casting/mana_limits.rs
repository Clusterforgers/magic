//! Goal: a mana ability that can be used once each turn (the Devotees, Kozilek's Translator) is a source of a payment that is used once and then waits for the next turn, at tables of two, three and four seats; the payment never chains it twice, and whole bot games with these sources finish and keep every invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::{Arc, Mutex};

use common::pid;
use common::seeds::for_each_seed;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Normal;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, PlayOptions, Scenario, ScriptError, play_game_with, ready};
use rstest::rstest;

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

fn spell(cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// Seat 0 holds three spells of `cost` and has `mine` in play; every seat has a library to draw from.
fn table(seats: usize, cost: &str, mine: &[&str]) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, [spell(cost), spell(cost), spell(cost)])
        .battlefield(0, mine.iter().map(|name| ready(definition(name))));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| plains()));
    }
    Driver::from_scenario(scenario)
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

/// Casts a spell whenever one is castable and counts the casts this turn.
fn casts_this_turn(driver: &mut Driver) -> usize {
    let mut casts = 0;
    while castable(driver) {
        driver.cast("Spell").unwrap();
        casts += 1;
    }
    casts
}

#[rstest]
#[case::one_devotee_pays_once("{R}", &["Plains", "Plains", "Mardu Devotee"], 1)]
#[case::the_land_is_spent_too("{R}", &["Plains", "Mardu Devotee"], 1)]
#[case::two_devotees_pay_twice("{R}", &["Plains", "Plains", "Mardu Devotee", "Mardu Devotee"], 2)]
#[case::a_land_beside_the_devotee("{R}", &["Mountain", "Plains", "Mardu Devotee"], 2)]
#[case::two_colours_of_two_devotees("{R}{G}", &["Plains", "Plains", "Mardu Devotee", "Temur Devotee"], 1)]
#[case::a_devotee_cannot_pay_the_color_it_lacks("{U}", &["Plains", "Mardu Devotee"], 0)]
#[case::the_translator_pays_with_life_once("{1}", &["Kozilek's Translator"], 1)]
#[case::the_translator_and_a_land("{1}", &["Kozilek's Translator", "Plains"], 2)]
#[case::a_devotee_needs_other_mana("{R}", &["Mardu Devotee"], 0)]
fn a_mana_ability_used_once_each_turn_pays_once_then_waits_for_the_next_turn(
    #[case] cost: &str,
    #[case] mine: &[&str],
    #[case] casts: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, cost, mine)?;
    assert_eq!(casts_this_turn(&mut driver), casts);
    if casts == 0 {
        return Ok(());
    }
    // The next turn of seat 0 frees the ability and untaps the lands.
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Main1)?;
    assert!(castable(&driver), "the ability is free again");
    assert_eq!(casts_this_turn(&mut driver), casts.min(3 - casts));
    Ok(())
}

#[test]
fn using_it_pays_the_cost_taps_nothing_and_logs_one_use() -> Result<(), ScriptError> {
    let mut driver = table(2, "{R}", &["Plains", "Mardu Devotee"])?;
    let events = driver.cast("Spell")?;
    let uses = events
        .iter()
        .filter(|event| matches!(event, Event::AbilityActivated { .. }))
        .count();
    // Cast and use of the ability: one `AbilityActivated` for the spell's cast does not exist, the mana ability's does.
    assert_eq!(uses, 1, "{events:?}");
    let devotee = driver.id_of("Mardu Devotee")?;
    assert!(
        !driver
            .state()
            .object(devotee)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20);
    Ok(())
}

#[test]
fn the_translator_pays_one_life_each_time_it_is_used() -> Result<(), ScriptError> {
    let mut driver = table(2, "{1}", &["Kozilek's Translator"])?;
    driver.cast("Spell")?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 19);
    Ok(())
}

fn deck() -> Vec<Arc<CardDefinition>> {
    [
        ("Forest", 12),
        ("Island", 5),
        ("Mountain", 5),
        ("Plains", 5),
        ("Swamp", 4),
        ("Mardu Devotee", 3),
        ("Temur Devotee", 3),
        ("Sultai Devotee", 3),
        ("Kozilek's Translator", 3),
        ("Llanowar Elves", 2),
        ("Grizzly Bears", 4),
        ("Serra Angel", 2),
        ("Giant Growth", 3),
        ("Lightning Bolt", 3),
        ("Counterspell", 3),
    ]
    .into_iter()
    .flat_map(|(name, copies)| (0..copies).map(move |_| definition(name)))
    .collect()
}

/// The uses of a mana ability seen in a campaign: an activation with no ability put on the stack after it.
fn limited_uses(events: &[Event]) -> u64 {
    events
        .windows(2)
        .filter(|pair| {
            matches!(pair[0], Event::AbilityActivated { .. })
                && !matches!(pair[1], Event::AbilityPutOnStack(_))
        })
        .count() as u64
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_these_sources_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(0u64);
    let games: u64 = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    for_each_seed(0..games, |seed| {
        let decks = (0..players).map(|_| deck()).collect();
        let setup = GameSetup::new(Arc::new(Normal), seed, decks);
        let options = PlayOptions {
            partial_payments: seed % 2 == 0,
            ..PlayOptions::default()
        };
        let outcome =
            play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
        *seen.lock().unwrap() += limited_uses(&outcome.events);
    });
    assert!(
        seen.into_inner().unwrap() > 0,
        "the games should use these abilities"
    );
}
