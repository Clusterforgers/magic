//! Goal: a card taken at random from a graveyard (Surreal Memoir) comes from the game's seeded random source at tables of two, three and four seats: the same seed gives the same card whatever the table size, every instant or sorcery card of the graveyard comes back for some seed and a creature card never, the card goes to the hand and the others stay, an empty choice is no event and spends no randomness, the pick is one public event and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;

use common::{names, pid};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The instants of seat 0's graveyard, and a creature card beside them.
const INSTANTS: [&str; 3] = ["Lightning Bolt", "Giant Growth", "Counterspell"];

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 holds Surreal Memoir with four Mountains and has `graveyard` in its graveyard.
fn table(seats: usize, seed: u64, graveyard: &[&str]) -> Scenario {
    Scenario::new(seats)
        .seed(seed)
        .hand_named(0, ["Surreal Memoir"])
        .battlefield(0, (0..4).map(|_| ready(mountain())))
        .graveyard_named(0, graveyard)
}

/// Casts the spell and returns the driver after everything resolved.
fn played(seats: usize, seed: u64, graveyard: &[&str]) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, seed, graveyard))?;
    driver.cast("Surreal Memoir")?;
    settle(&mut driver)?;
    Ok(driver)
}

/// The instant that came to the hand, when one did.
fn returned(driver: &Driver) -> Option<String> {
    names(driver.state(), Zone::Hand(pid(0)))
        .into_iter()
        .find(|name| INSTANTS.contains(&name.as_str()))
}

fn picks(driver: &Driver) -> Vec<Vec<mtg_core::ids::ObjectId>> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::ChosenAtRandom { cards } => Some(cards.clone()),
            _ => None,
        })
        .collect()
}

#[rstest]
fn the_same_seed_gives_the_same_card_at_every_table_size(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut graveyard = INSTANTS.to_vec();
    graveyard.push("Grizzly Bears");
    let mut seen = BTreeSet::new();
    for seed in 0..40 {
        let driver = played(seats, seed, &graveyard)?;
        let card = returned(&driver).expect("an instant came back");
        assert_eq!(
            Some(card.clone()),
            returned(&played(2, seed, &graveyard)?),
            "seed {seed}"
        );
        assert_eq!(
            Some(card.clone()),
            returned(&played(seats, seed, &graveyard)?),
            "seed {seed} twice"
        );
        // The creature stays, the card that came back left the graveyard and the others stayed.
        let grave = names(driver.state(), Zone::Graveyard(pid(0)));
        assert!(grave.contains(&"Grizzly Bears".to_owned()), "seed {seed}");
        assert!(!grave.contains(&card), "seed {seed}");
        assert_eq!(picks(&driver).len(), 1, "one public event for the pick");
        let replayed = replay(
            &table(seats, seed, &graveyard).setup().unwrap(),
            driver.engine().events(),
        )
        .unwrap();
        assert_eq!(replayed.state_hash(), driver.state().state_hash());
        seen.insert(card);
    }
    assert_eq!(
        seen,
        INSTANTS.iter().map(|name| (*name).to_owned()).collect(),
        "every option comes for some seed"
    );
    Ok(())
}

#[rstest]
#[case::one_option(&["Lightning Bolt"])]
#[case::two_options(&["Lightning Bolt", "Counterspell"])]
fn the_card_is_the_only_one_that_can_be_picked(
    #[case] graveyard: &[&str],
) -> Result<(), ScriptError> {
    for seed in 0..12 {
        let driver = played(2, seed, graveyard)?;
        let card = returned(&driver).expect("an instant came back");
        assert!(graveyard.contains(&card.as_str()));
    }
    Ok(())
}

#[rstest]
#[case::only_a_creature(&["Grizzly Bears"])]
#[case::nothing(&[])]
fn nothing_to_pick_is_no_event_and_spends_no_randomness(
    #[case] graveyard: &[&str],
) -> Result<(), ScriptError> {
    let start = Driver::from_scenario(table(2, 5, graveyard))?;
    let before = start.state().rng_index();
    let driver = played(2, 5, graveyard)?;
    assert_eq!(picks(&driver), Vec::<Vec<mtg_core::ids::ObjectId>>::new());
    assert_eq!(returned(&driver), None);
    assert_eq!(driver.state().rng_index(), before);
    Ok(())
}

#[test]
fn the_pick_spends_one_substream() -> Result<(), ScriptError> {
    let start = Driver::from_scenario(table(2, 5, &INSTANTS))?;
    let before = start.state().rng_index();
    let driver = played(2, 5, &INSTANTS)?;
    assert_eq!(driver.state().rng_index(), before + 1);
    Ok(())
}
