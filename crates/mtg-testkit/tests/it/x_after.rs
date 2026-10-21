//! Goal: the X of a spell outlives its resolution where the rules say: Walking Ballista and Hangarback Walker enter with the X they were cast with (0 counters for X = 0), Hangarback Walker's death leaves a Thopter for each counter, Gadwick draws the X it was cast with even when it left before its trigger resolved, Disembowel and Forced March read X in what they may target or destroy, a countered spell puts nothing onto the battlefield, and the log of each replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, pid};
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{NumberChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{free_creature, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn lands(
    count: usize,
    land: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

fn say(driver: &mut Driver, x: u32) -> Result<(), ScriptError> {
    driver.expect_decision(0, "choose-number")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    Ok(())
}

fn most_x(driver: &Driver) -> Option<u32> {
    match &driver.decision().unwrap().kind {
        DecisionKind::ChooseNumber(options) => options.max,
        _ => None,
    }
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

fn count(driver: &Driver, zone: Zone, name: &str) -> usize {
    names(driver.state(), zone)
        .iter()
        .filter(|found| *found == name)
        .count()
}

/// The +1/+1 counters on the first permanent named `name`.
fn counters(driver: &Driver, name: &str) -> Option<u32> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|object| object.card().name == name)
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&CounterKind::PlusOnePlusOne))
}

fn replays(driver: &Driver, scenario: Scenario) {
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Casts `name` for `x` and lets everything resolve.
fn cast(driver: &mut Driver, name: &str, x: u32) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    say(driver, x)?;
    pay(driver)?;
    settle(driver)
}

#[rstest]
#[case::nothing_dies_as_a_0_0("Walking Ballista", 0, None)]
#[case::one("Walking Ballista", 1, Some(1))]
#[case::three("Walking Ballista", 3, Some(3))]
#[case::walker_nothing("Hangarback Walker", 0, None)]
#[case::walker_one("Hangarback Walker", 1, Some(1))]
#[case::walker_three("Hangarback Walker", 3, Some(3))]
fn an_artifact_enters_with_the_x_it_was_cast_with(
    #[case] name: &str,
    #[case] x: u32,
    #[case] counters_on_it: Option<u32>,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [card(name).unwrap()])
            .battlefield(0, lands(6, mountain))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast(name)?;
    assert_eq!(most_x(&driver), Some(3), "{{X}}{{X}} of six lands");
    say(&mut driver, x)?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    assert_eq!(counters(&driver, name), counters_on_it);
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::one(1)]
#[case::three(3)]
fn a_hangarback_walker_that_dies_leaves_a_thopter_for_each_counter(
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [card("Hangarback Walker").unwrap()])
            .battlefield(0, lands(6, mountain))
            .hand(1, [card("Doom Blade").unwrap()])
            .battlefield(1, lands(2, swamp))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    cast(&mut driver, "Hangarback Walker", x)?;
    assert_eq!(counters(&driver, "Hangarback Walker"), Some(x));
    driver.pass()?;
    driver.expect_decision(1, "priority")?;
    let walker = driver.target_named("Hangarback Walker")?;
    driver.begin_cast("Doom Blade")?;
    driver.choose_targets(&[walker])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(count(&driver, Zone::Battlefield, "Hangarback Walker"), 0);
    assert_eq!(
        count(&driver, Zone::Battlefield, "Thopter"),
        usize::try_from(x).unwrap()
    );
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::nothing(0)]
#[case::one(1)]
#[case::three(3)]
fn gadwick_draws_the_x_it_was_cast_with(#[case] x: u32) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [card("Gadwick, the Wizened").unwrap()])
            .battlefield(0, lands(6, island))
            .library(0, (0..10).map(|_| free_creature("Filler", 1, 1)))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let before = driver.state().zone(Zone::Hand(pid(0))).len();
    cast(&mut driver, "Gadwick, the Wizened", x)?;
    let drawn = driver.state().zone(Zone::Hand(pid(0))).len() + 1 - before;
    assert_eq!(drawn, usize::try_from(x).unwrap());
    replays(&driver, scenario());
    Ok(())
}

#[test]
fn the_trigger_of_a_permanent_that_left_still_reads_its_x() -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [card("Gadwick, the Wizened").unwrap()])
            .battlefield(0, lands(6, island))
            .library(0, (0..10).map(|_| free_creature("Filler", 1, 1)))
            .hand(1, [card("Doom Blade").unwrap()])
            .battlefield(1, lands(2, swamp))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let before = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_cast("Gadwick, the Wizened")?;
    say(&mut driver, 3)?;
    pay(&mut driver)?;
    driver.pass()?;
    driver.pass()?;
    // The enters trigger waits; its controller passes and the opponent destroys Gadwick in response.
    driver.pass()?;
    driver.expect_decision(1, "priority")?;
    let gadwick = driver.target_named("Gadwick, the Wizened")?;
    driver.begin_cast("Doom Blade")?;
    driver.choose_targets(&[gadwick])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(count(&driver, Zone::Battlefield, "Gadwick, the Wizened"), 0);
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len() + 1 - before,
        3
    );
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::mana_value_one_is_a_dork(1, "Elf")]
#[case::mana_value_two_is_the_bear(2, "Bear")]
fn disembowel_destroys_only_a_creature_whose_mana_value_is_x(
    #[case] x: u32,
    #[case] dies: &str,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [card("Disembowel").unwrap()])
            .battlefield(0, lands(4, swamp))
            .battlefield(
                1,
                [
                    ready(free_creature_of("Elf", "{G}")),
                    ready(free_creature_of("Bear", "{1}{G}")),
                ],
            )
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Disembowel")?;
    say(&mut driver, x)?;
    let wanted = driver.target_named(dies)?;
    driver.choose_targets(&[wanted])?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    let left = names(driver.state(), Zone::Battlefield);
    assert!(!left.contains(&dies.to_owned()));
    assert_eq!(
        left.iter().filter(|n| *n == "Elf" || *n == "Bear").count(),
        1
    );
    replays(&driver, scenario());
    Ok(())
}

#[test]
fn disembowel_is_offered_before_x_is_chosen_and_only_the_creature_of_that_value_is_a_target()
-> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .hand(0, [card("Disembowel").unwrap()])
        .battlefield(0, lands(4, swamp))
        .battlefield(1, [ready(free_creature_of("Bear", "{1}{G}"))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Disembowel")?;
    say(&mut driver, 0)?;
    // No creature has mana value 0: the cast is backed out of as it would be for any spell without a target.
    assert!(driver.expect_decision(0, "priority").is_ok());
    Ok(())
}

#[rstest]
#[case::zero(0, 2)]
#[case::one(1, 1)]
#[case::two(2, 0)]
#[case::large(3, 0)]
fn forced_march_destroys_each_creature_whose_mana_value_is_x_or_less(
    #[case] x: u32,
    #[case] left: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [card("Forced March").unwrap()])
            .battlefield(0, lands(6, swamp))
            .battlefield(
                1,
                [
                    ready(free_creature("Token", 1, 1)),
                    ready(free_creature_of("Elf", "{G}")),
                    ready(free_creature_of("Bear", "{1}{G}")),
                ],
            )
    };
    // The token-like creature has no mana cost: mana value 0.
    let mut driver = Driver::from_scenario(scenario())?;
    cast(&mut driver, "Forced March", x)?;
    let alive = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|n| matches!(n.as_str(), "Token" | "Elf" | "Bear"))
        .count();
    let expected = match x {
        0 => 2,
        1 => 1,
        _ => left,
    };
    assert_eq!(alive, expected);
    replays(&driver, scenario());
    Ok(())
}

#[test]
fn a_countered_ballista_puts_nothing_onto_the_battlefield() -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [card("Walking Ballista").unwrap()])
            .battlefield(0, lands(6, mountain))
            .hand(1, [card("Counterspell").unwrap()])
            .battlefield(1, lands(2, island))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Walking Ballista")?;
    say(&mut driver, 3)?;
    pay(&mut driver)?;
    driver.pass()?;
    driver.expect_decision(1, "priority")?;
    let top = *driver.state().zone(Zone::Stack).last().unwrap();
    let spell = Target::Object(driver.state().object(top).unwrap().reference());
    driver.begin_cast("Counterspell")?;
    driver.choose_targets(&[spell])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(count(&driver, Zone::Battlefield, "Walking Ballista"), 0);
    assert_eq!(
        count(&driver, Zone::Graveyard(pid(0)), "Walking Ballista"),
        1
    );
    replays(&driver, scenario());
    Ok(())
}

#[test]
fn bot_games_with_the_x_after_cards_keep_every_invariant_and_use_what_x_leaves_behind() {
    use crate::common::seeds::for_each_seed;
    use mtg_engine::Event;
    use mtg_testkit::{PlayOptions, play_game_with, x_after_fuzz_setup};

    let seen = std::sync::Mutex::new((0u64, 0u64, 0u64));
    let games = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                x_after_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    Event::SpellCast { choices, .. } if choices.x.is_some_and(|x| x > 0) => {
                        seen.0 += 1;
                    }
                    Event::AbilityPutOnStack(put)
                        if put.cause.is_some() && put.choices.x.is_some_and(|x| x > 0) =>
                    {
                        seen.1 += 1;
                    }
                    Event::CountersChanged(change)
                        if change.counter == CounterKind::PlusOnePlusOne && change.delta > 1 =>
                    {
                        seen.2 += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let (spells, triggers, counters) = seen.into_inner().unwrap();
    assert!(spells > 0, "some spell was cast with X above nothing");
    assert!(triggers > 0, "some trigger carried the X of its spell");
    assert!(counters > 0, "some permanent entered with several counters");
}

/// A creature with a mana cost, for a mana value to compare with.
fn free_creature_of(name: &str, cost: &str) -> std::sync::Arc<mtg_cards::CardDefinition> {
    let base = free_creature(name, 2, 2);
    let mut card = (*base).clone();
    card.mana_cost = Some(mtg_core::mana::ManaCost::parse(cost).unwrap());
    std::sync::Arc::new(card)
}
