//! Goal: a spell that takes exactly X targets (Distorting Wake, Entreat the Dead) asks for X first and then for exactly that many targets, at tables of two, three and four seats: X of nothing asks for no target, a count other than X is refused, a value of X that leaves too few legal targets is not offered, the cards go where the spell says, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{free_creature, island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn lands(count: usize, land: fn() -> Arc<CardDefinition>) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

fn say(driver: &mut Driver, x: u32) -> Result<(), ScriptError> {
    driver.expect_decision(0, "choose-number")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    Ok(())
}

fn pay(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    Ok(())
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn replays(driver: &Driver, scenario: Scenario) {
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Seat 0 holds Distorting Wake with `islands` Islands; every other seat has `each` creatures called "Foe <seat> <n>".
fn wake(seats: usize, islands: usize, each: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Distorting Wake").unwrap()])
        .battlefield(0, lands(islands, island));
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            (0..each).map(|n| ready(free_creature(&format!("Foe {seat} {n}"), 2, 2))),
        );
    }
    scenario
}

/// The names of the creatures on the battlefield of every seat but the first.
fn foes(driver: &Driver, seats: usize) -> Vec<String> {
    (1..seats)
        .flat_map(|seat| names(driver.state(), Zone::Battlefield))
        .filter(|name| name.starts_with("Foe"))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[rstest]
fn x_targets_are_asked_for_exactly_x_and_returned(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] x: u32,
) -> Result<(), ScriptError> {
    let scenario = || wake(seats, 6, 2);
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Distorting Wake")?;
    if x as usize > (seats - 1) * 2 {
        // Too few legal targets: that value of X is not offered.
        assert!(say(&mut driver, x).is_err());
        return Ok(());
    }
    say(&mut driver, x)?;
    if x > 0 {
        let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
            panic!("the targets are asked after X");
        };
        assert_eq!((options.min, options.max), (x, x), "exactly X targets");
        let legal = options.legal.clone();
        // One too few or one too many is refused, the right count is taken.
        let below: Vec<_> = legal.iter().copied().take(x as usize - 1).collect();
        if x > 1 {
            assert!(driver.choose_targets(&below).is_err());
        }
        if legal.len() > x as usize {
            let above: Vec<_> = legal.iter().copied().take(x as usize + 1).collect();
            assert!(driver.choose_targets(&above).is_err());
        }
        let exact: Vec<_> = legal.iter().copied().take(x as usize).collect();
        driver.choose_targets(&exact)?;
    }
    pay(&mut driver)?;
    settle(&mut driver)?;
    let returned: usize = (1..seats)
        .map(|seat| {
            names(driver.state(), Zone::Hand(pid(seat)))
                .iter()
                .filter(|name| name.starts_with("Foe"))
                .count()
        })
        .sum();
    assert_eq!(returned, x as usize);
    assert_eq!(
        foes(&driver, seats).len(),
        (seats - 1) * 2 - x as usize,
        "the rest stay"
    );
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::one_target_for_more_mana(2, 6, 1, 1)]
#[case::two_targets_for_more_mana(3, 6, 1, 2)]
#[case::mana_is_the_limit(2, 4, 3, 1)]
#[case::both_limits(4, 9, 1, 3)]
fn a_value_of_x_without_enough_legal_targets_is_not_offered(
    #[case] seats: usize,
    #[case] islands: usize,
    #[case] each: usize,
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(wake(seats, islands, each))?;
    driver.begin_cast("Distorting Wake")?;
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("X is asked when more than one value is left");
    };
    assert_eq!((options.min, options.max), (0, Some(most)));
    let too_many = Answer::ChooseNumber(NumberChoice::Number(most + 1));
    assert!(driver.answer(too_many).is_err());
    Ok(())
}

#[test]
fn nothing_to_return_means_a_spell_that_asks_only_for_zero() -> Result<(), ScriptError> {
    let scenario = || wake(2, 3, 0);
    let mut driver = Driver::from_scenario(scenario())?;
    // No creature is in play, so X can only be 0: no question for X, no target decision.
    driver.begin_cast("Distorting Wake")?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Distorting Wake"]
    );
    replays(&driver, scenario());
    Ok(())
}

/// Seat 0 casts Entreat the Dead with `lands` Swamps and has the creature cards `graveyard` in its graveyard.
fn entreat(seats: usize, lands_count: usize, graveyard: &[&str]) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Entreat the Dead").unwrap()])
        .battlefield(0, lands(lands_count, swamp));
    scenario = scenario.graveyard(0, graveyard.iter().map(|name| free_creature(name, 1, 1)));
    scenario
}

#[rstest]
fn entreat_the_dead_returns_x_creature_cards_from_your_graveyard(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] x: u32,
) -> Result<(), ScriptError> {
    // Two cards in the graveyard and mana for two: X is 0, 1 or 2.
    let scenario = || entreat(seats, 7, &["Dead A", "Dead B"]);
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Entreat the Dead")?;
    say(&mut driver, x)?;
    if x > 0 {
        let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
            panic!("the targets are asked after X");
        };
        assert_eq!((options.min, options.max), (x, x));
        let exact: Vec<_> = options.legal.iter().copied().take(x as usize).collect();
        driver.choose_targets(&exact)?;
    }
    pay(&mut driver)?;
    settle(&mut driver)?;
    let back = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| name.starts_with("Dead"))
        .count();
    assert_eq!(back, x as usize);
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0)))
            .iter()
            .filter(|name| name.starts_with("Dead"))
            .count(),
        2 - x as usize
    );
    replays(&driver, scenario());
    Ok(())
}

#[test]
fn entreat_the_dead_cannot_ask_for_more_cards_than_the_graveyard_holds() -> Result<(), ScriptError>
{
    // One creature card and mana for X up to three: only 0 and 1 are offered.
    let mut driver = Driver::from_scenario(entreat(2, 9, &["Dead A"]))?;
    driver.begin_cast("Entreat the Dead")?;
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("X is asked");
    };
    assert_eq!((options.min, options.max), (0, Some(1)));
    Ok(())
}
