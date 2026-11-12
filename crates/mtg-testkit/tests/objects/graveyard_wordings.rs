//! Goal: the abilities a card has in its graveyard play at tables of two, three and four seats: a card activates its own ability from the graveyard and returns to the battlefield tapped (Drownyard Temple) or to its owner's hand (Sanitarium Skeleton) while a card of the same name in a hand does nothing, an Aura put into a graveyard from the battlefield returns to its owner's hand (Rancor), a card put into a graveyard from the battlefield is shuffled into its owner's library (Serra Avatar) and not left in the graveyard, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{lands, pay};
use crate::common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, replay};
use mtg_testkit::cards::{free_creature, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, damaged, ready};
use rstest::rstest;

/// Answers passively until a player holds priority with nothing on the stack and the state-based actions have run.
fn settle_all(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack did not empty".into()))
}

/// Passes until `done` holds of the state.
fn until(driver: &mut Driver, done: impl Fn(&GameState) -> bool) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if done(driver.state()) {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    let state = driver.state();
    let seen = format!(
        "bf {:?} gy {:?} hand {:?} stack {:?} turn {}",
        names(state, Zone::Battlefield),
        names(state, Zone::Graveyard(pid(0))),
        names(state, Zone::Hand(pid(0))),
        state.stack().len(),
        state.turn().number()
    );
    Err(ScriptError::Stuck(seen))
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0 holds `card` in its graveyard with `count` lands of `land` in play.
fn graveyard_game(
    seats: usize,
    card: &str,
    land: fn() -> Arc<CardDefinition>,
    count: usize,
) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(seats)
            .graveyard_named(0, [card])
            .battlefield(0, lands(count, land)),
    )
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn a_land_returns_itself_from_the_graveyard_to_the_battlefield_tapped(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = graveyard_game(seats, "Drownyard Temple", mountain, 3)?;
    driver.begin_activation("Drownyard Temple")?;
    pay(&mut driver)?;
    settle_all(&mut driver)?;
    let state = driver.state();
    assert_eq!(names(state, Zone::Graveyard(pid(0))), Vec::<String>::new());
    let temple = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == "Drownyard Temple")
        .expect("the Temple is in play");
    assert!(state.object(temple).unwrap().permanent().unwrap().tapped());
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn a_creature_returns_itself_from_the_graveyard_to_its_owners_hand(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = graveyard_game(seats, "Sanitarium Skeleton", swamp, 3)?;
    driver.begin_activation("Sanitarium Skeleton")?;
    pay(&mut driver)?;
    settle_all(&mut driver)?;
    let state = driver.state();
    assert_eq!(names(state, Zone::Hand(pid(0))), ["Sanitarium Skeleton"]);
    assert_eq!(names(state, Zone::Graveyard(pid(0))), Vec::<String>::new());
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn the_ability_works_from_the_graveyard_only(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Sanitarium Skeleton"])
            .battlefield(0, lands(3, swamp)),
    )?;
    assert_eq!(
        driver.offered_abilities("Sanitarium Skeleton")?,
        Vec::<usize>::new(),
        "a card in a hand has no such ability"
    );
    assert!(driver.begin_activation("Sanitarium Skeleton").is_err());
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn an_aura_that_dies_with_its_host_returns_to_its_owners_hand(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(
        0,
        [
            damaged(free_creature("Bear", 2, 2), 2),
            attached_to(ready(card("Rancor").unwrap()), 0, 0),
        ],
    ))?;
    until(&mut driver, |state| {
        names(state, Zone::Hand(pid(0))).contains(&"Rancor".to_owned())
    })?;
    let state = driver.state();
    assert_eq!(names(state, Zone::Graveyard(pid(0))), ["Bear"]);
    assert_eq!(names(state, Zone::Battlefield), Vec::<String>::new());
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn a_card_put_into_a_graveyard_from_the_battlefield_is_shuffled_into_its_owners_library(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats).battlefield(0, [damaged(card("Serra Avatar").unwrap(), 99)]),
    )?;
    let before = driver.state().zone(Zone::Library(pid(0))).len();
    until(&mut driver, |state| {
        names(state, Zone::Library(pid(0))).contains(&"Serra Avatar".to_owned())
    })?;
    settle_all(&mut driver)?;
    let state = driver.state();
    assert_eq!(names(state, Zone::Graveyard(pid(0))), Vec::<String>::new());
    assert_eq!(state.zone(Zone::Library(pid(0))).len(), before + 1);
    assert_replays(&driver);
    Ok(())
}
