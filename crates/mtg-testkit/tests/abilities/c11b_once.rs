//! Goal: a permission to cast a kind of spell from the graveyard once during each of its controller's turns plays as
//! printed at tables of two, three and four seats: Karador and Gisa and Geralf let their controller cast one matching
//! card from their graveyard on their turn, a second one waits for their next turn, a card of another kind never comes,
//! and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::names;

/// Answers passively until the stack and the waiting triggers are empty.
fn c11b_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let state = driver.state();
        if state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// A Zombie creature card for `{1}{B}`.
fn c11b_zombie() -> Arc<CardDefinition> {
    card("Dregscape Zombie").unwrap()
}

/// Seat 0 controls the permanent `name` and six lands and has two copies of `kind` in its graveyard; every other seat
/// has an Island.
fn c11b_table(seats: usize, name: &str, kind: fn() -> Arc<CardDefinition>) -> Scenario {
    let mut mine = vec![ready(card(name).unwrap())];
    mine.extend((0..3).map(|_| ready(forest())));
    mine.extend((0..3).map(|_| ready(swamp())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .graveyard(0, [kind(), kind()]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(island())]);
    }
    scenario
}

/// The name of the card a `kind` makes.
fn c11b_name(kind: fn() -> Arc<CardDefinition>) -> String {
    kind().name.clone()
}

#[rstest]
#[case::karador("Karador, Ghost Chieftain", bear as fn() -> Arc<CardDefinition>)]
#[case::gisa_and_geralf("Gisa and Geralf", c11b_zombie as fn() -> Arc<CardDefinition>)]
fn c11b_one_matching_card_comes_from_the_graveyard_each_of_your_turns(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] kind: fn() -> Arc<CardDefinition>,
) -> Result<(), ScriptError> {
    let scenario = c11b_table(seats, name, kind);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let creature = c11b_name(kind);
    driver.begin_cast(&creature)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    c11b_settle(&mut driver)?;
    let graveyard = Zone::Graveyard(PlayerId(0));
    assert_eq!(
        names(driver.state(), graveyard),
        std::slice::from_ref(&creature)
    );
    assert!(
        driver.begin_cast(&creature).is_err(),
        "a second card waits for the next turn"
    );
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    driver.begin_cast(&creature)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    c11b_settle(&mut driver)?;
    assert_eq!(names(driver.state(), graveyard), Vec::<String>::new());
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn c11b_a_card_of_another_kind_stays_in_the_graveyard(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(c11b_table(seats, "Gisa and Geralf", bear))?;
    assert!(driver.begin_cast("Grizzly Bears").is_err());
    let mut driver = Driver::from_scenario(c11b_table(seats, "Grizzly Bears", bear))?;
    assert!(
        driver.begin_cast("Grizzly Bears").is_err(),
        "nothing lets a card come from the graveyard without the permission"
    );
    Ok(())
}
