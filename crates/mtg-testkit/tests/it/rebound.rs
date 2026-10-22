//! Goal: rebound exiles a spell cast from its owner's hand as it resolves and lets its controller cast it for free at the beginning of their next upkeep and only then: a spell that was countered, copied, cast from another zone or whose cast is declined or has nothing to aim at is never cast again from exile, the second cast does not rebound, Rest in Peace lets the controller pick the exile of rebound, and the log of the game replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, pid};
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{Target, YesNoChoice};
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState, replay};
use mtg_testkit::cards::{island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The object called `name` in `zone`.
fn find(state: &GameState, zone: Zone, name: &str) -> Option<ObjectId> {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == name)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// The log replays to the state the game is in.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Answers what a resolution asks until the stack is empty and a player has priority: `cast` says whether a free cast
/// is accepted, targets are player `aim`.
fn settle(driver: &mut Driver, cast: bool, aim: usize) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    for _ in 0..60 {
        match driver.decision()?.kind.name() {
            "yes-no" => events.extend(driver.answer(Answer::YesNo(YesNoChoice(cast)))?),
            "choose-targets" => {
                let target = driver.target_player(aim);
                events.extend(driver.choose_targets(&[target])?);
            }
            "pay-mana" => events.extend(driver.answer(Answer::PayMana(PayChoice::Auto))?),
            "priority" if driver.state().zone(Zone::Stack).is_empty() => return Ok(events),
            "priority" => events.extend(driver.pass()?),
            _ => events.extend(driver.answer_passively()?),
        }
    }
    Err(ScriptError::Stuck("the rebounding spell".to_owned()))
}

/// Seat 0 holds Staggershock and three Mountains in the first main phase of turn 1.
fn game() -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Staggershock"])
            .battlefield(0, [ready(mountain()), ready(mountain()), ready(mountain())]),
    )
}

/// Seat 0 casts Staggershock at seat 1 and the spell resolves.
fn cast_and_resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    let target = driver.target_player(1);
    driver.cast_at("Staggershock", &[target])?;
    settle(driver, true, 1)?;
    Ok(())
}

#[test]
fn the_spell_is_exiled_as_it_resolves_and_cast_for_free_at_the_next_upkeep() -> Result<(), ScriptError> {
    let mut driver = game()?;
    cast_and_resolve(&mut driver)?;
    assert_eq!(life(&driver, 1), 18);
    assert!(find(driver.state(), Zone::Exile, "Staggershock").is_some());
    assert!(find(driver.state(), Zone::Graveyard(pid(0)), "Staggershock").is_none());
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(life(&driver, 1), 18, "the upkeep of the other player casts nothing");
    driver.idle_until(3, Step::Upkeep)?;
    let events = settle(&mut driver, true, 1)?;
    assert_eq!(life(&driver, 1), 16, "the second cast dealt 2 damage");
    assert!(events.iter().any(|e| matches!(
        e,
        Event::SpellCast { choices, .. } if choices.way == Some(mtg_cards::value::CastWay::Free)
    )));
    assert!(find(driver.state(), Zone::Exile, "Staggershock").is_none());
    assert!(
        find(driver.state(), Zone::Graveyard(pid(0)), "Staggershock").is_some(),
        "cast from exile, the spell does not rebound again"
    );
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::declined(false)]
fn a_declined_cast_leaves_the_card_in_exile_for_good(#[case] accept: bool) -> Result<(), ScriptError> {
    let mut driver = game()?;
    cast_and_resolve(&mut driver)?;
    driver.idle_until(3, Step::Upkeep)?;
    settle(&mut driver, accept, 1)?;
    assert_eq!(life(&driver, 1), 18);
    assert!(find(driver.state(), Zone::Exile, "Staggershock").is_some());
    driver.idle_until(5, Step::Upkeep)?;
    assert!(
        driver.state().zone(Zone::Stack).is_empty(),
        "rebound happens once"
    );
    assert!(find(driver.state(), Zone::Exile, "Staggershock").is_some());
    assert_replays(&driver);
    Ok(())
}

#[test]
fn a_spell_that_is_countered_goes_to_the_graveyard_and_does_not_rebound() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Staggershock"])
            .hand_named(1, ["Counterspell"])
            .battlefield(0, [ready(mountain()), ready(mountain()), ready(mountain())])
            .battlefield(1, [ready(island()), ready(island())]),
    )?;
    let target = driver.target_player(1);
    driver.cast_at("Staggershock", &[target])?;
    driver.pass()?;
    let spell = find(driver.state(), Zone::Stack, "Staggershock").unwrap();
    let at = Target::Object(driver.state().object(spell).unwrap().reference());
    driver.cast_at("Counterspell", &[at])?;
    settle(&mut driver, true, 1)?;
    assert!(find(driver.state(), Zone::Graveyard(pid(0)), "Staggershock").is_some());
    assert!(find(driver.state(), Zone::Exile, "Staggershock").is_none());
    driver.idle_until(3, Step::Upkeep)?;
    assert_eq!(life(&driver, 1), 20);
    Ok(())
}
