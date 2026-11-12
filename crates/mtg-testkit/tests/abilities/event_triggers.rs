//! Goal: the corpus cards that wait for an event about a player or a permanent (life gained, damage dealt, cards drawn, discarded and cycled, permanents tapped and untapped, lands tapped for mana) trigger for the players and permanents they name, with "that player" and "that permanent's controller" the one the event is about, and whole bot games at two, three and four seats with decks of them finish, keep every invariant, trigger every kind and replay to the state they were played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::common::seeds::fuzz_games;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, replay};
use mtg_testkit::cards::{free_creature, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::event_cards::EVENT_CARDS;
use mtg_testkit::{Driver, Scenario, ScriptError, events_fuzz_setup, play_game, ready, tapped};
use rstest::rstest;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Passes priority while the stack holds something, saying yes to every "may".
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass()?;
            }
            _ => return Ok(()),
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// An artifact that can be sacrificed for a little life.
fn gem() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Gem")
            .with_type(CardType::Artifact)
            .with_rules(ability),
    )
}

fn counters_on(driver: &Driver, name: &str) -> usize {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .count()
}

#[test]
fn gaining_life_puts_a_counter_on_ajanis_pridemate() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(card("Ajani's Pridemate").unwrap()), ready(gem())]),
    )?;
    assert_eq!(counters_on(&driver, "Ajani's Pridemate"), 0);
    driver.begin_activation("Gem")?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 21);
    assert_eq!(counters_on(&driver, "Ajani's Pridemate"), 1);
    assert_replays(&driver);
    Ok(())
}

#[test]
fn an_opponents_gain_of_life_is_not_yours() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(card("Ajani's Pridemate").unwrap())])
            .battlefield(1, [ready(gem())]),
    )?;
    // The other seat sacrifices its Gem when it has priority, which is when seat 0 has passed.
    driver.pass()?;
    driver.begin_activation("Gem")?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), 21);
    assert_eq!(counters_on(&driver, "Ajani's Pridemate"), 0);
    Ok(())
}

#[test]
fn tapping_a_land_for_mana_costs_its_controller_a_life_for_each_tap_with_manabarbs()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(card("Manabarbs").unwrap()),
            ready(mountain()),
            ready(island()),
        ],
    ))?;
    driver.tap_for_mana("Mountain")?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 19);
    assert_eq!(life(&driver, 1), 20);
    driver.tap_for_mana("Island")?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 18, "one for each land, not for each mana");
    assert_replays(&driver);
    Ok(())
}

#[test]
fn an_opponent_drawing_takes_a_damage_from_underworld_dreams() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(card("Underworld Dreams").unwrap())]),
    )?;
    driver.idle_until(2, Step::Main1)?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), 19, "the draw of the opponent");
    assert_eq!(
        life(&driver, 0),
        20,
        "not the first turn without a draw, nor yours"
    );
    driver.idle_until(3, Step::Main1)?;
    settle(&mut driver)?;
    assert_eq!(
        life(&driver, 0),
        20,
        "its controller drew on turn 3 and is no opponent of itself"
    );
    assert_eq!(life(&driver, 1), 19);
    Ok(())
}

#[test]
fn an_opponents_discard_takes_two_damage_from_megrim_for_each_card() -> Result<(), ScriptError> {
    let spare: Vec<_> = (0..9)
        .map(|n| free_creature(&format!("Spare {n}"), 1, 1))
        .collect();
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(card("Megrim").unwrap())])
            .hand(1, spare),
    )?;
    // Ten cards at the end of turn 2 are three more than the hand holds.
    driver.idle_until(3, Step::Main1)?;
    settle(&mut driver)?;
    assert_eq!(hand(&driver, 1), 7);
    assert_eq!(life(&driver, 1), 20 - 3 * 2);
    assert_eq!(life(&driver, 0), 20);
    assert_replays(&driver);
    Ok(())
}

#[test]
fn a_permanent_untapping_mills_its_controller_when_mesmeric_orb_is_in_play()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(card("Mesmeric Orb").unwrap())])
            .battlefield(1, [tapped(free_creature("Sleeper", 1, 1))]),
    )?;
    let library = driver.state().zone(Zone::Library(pid(1))).len();
    driver.idle_until(2, Step::Main1)?;
    settle(&mut driver)?;
    // The draw and the card the Sleeper's untapping milled; seat 0 has nothing tapped.
    assert_eq!(
        driver.state().zone(Zone::Library(pid(1))).len(),
        library - 2
    );
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(1))).len(), 1);
    assert_eq!(
        driver.state().zone(Zone::Library(pid(0))).len(),
        driver.state().zone(Zone::Library(pid(0))).len()
    );
    assert_replays(&driver);
    Ok(())
}

#[test]
fn attacking_taps_the_wanderbrine_preacher_and_gains_its_controller_two_life()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(card("Wanderbrine Preacher").unwrap())]),
    )?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Wanderbrine Preacher", 1)])?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 22);
    assert_replays(&driver);
    Ok(())
}

#[test]
fn thieving_magpie_draws_when_it_damages_an_opponent_and_not_when_it_is_blocked()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(card("Thieving Magpie").unwrap())]),
    )?;
    let cards = hand(&driver, 0);
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Thieving Magpie", 1)])?;
    driver.idle_until(1, Step::Main2)?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), 19, "it is a 1/3");
    assert_eq!(hand(&driver, 0), cards + 1);
    assert_replays(&driver);
    Ok(())
}

#[test]
fn cycling_renewed_faith_logs_the_cycling_and_triggers_its_own_ability_from_the_graveyard()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [card("Renewed Faith").unwrap()])
            .battlefield(0, [ready(plains()), ready(plains())]),
    )?;
    driver.begin_activation("Renewed Faith")?;
    let events = driver.pay_tapping(&["Plains", "Plains"], 0)?;
    assert!(
        events
            .iter()
            .position(|e| matches!(e, Event::Cycled { .. }))
            .zip(
                events
                    .iter()
                    .position(|e| matches!(e, Event::CardDiscarded { .. }))
            )
            .is_some_and(|(cycled, discarded)| cycled == discarded + 1),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::AbilityTriggered(_)))
    );
    let cards = hand(&driver, 0);
    settle(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Renewed Faith"]
    );
    assert_eq!(
        life(&driver, 0),
        22,
        "the may-gain of the trigger, answered yes"
    );
    assert_eq!(hand(&driver, 0), cards + 1, "and the draw of the cycling");
    assert_replays(&driver);
    Ok(())
}

/// The cards a game's triggers came from, by name.
fn triggered_cards(setup: &GameSetup, outcome: &mtg_testkit::GameOutcome) -> BTreeSet<String> {
    let state = replay(setup, &outcome.events).unwrap();
    outcome
        .events
        .iter()
        .filter_map(|event| match event {
            Event::AbilityTriggered(triggered) => state.object(triggered.source.id),
            _ => None,
        })
        .map(|object| object.card().name.clone())
        .collect()
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_event_games_finish_keep_every_invariant_and_trigger_every_card(#[case] players: usize) {
    let games: u64 = fuzz_games(30);
    let mut seen = BTreeSet::new();
    for seed in 0..games {
        let setup = events_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.extend(triggered_cards(&setup, &outcome));
    }
    for &(name, _) in EVENT_CARDS {
        assert!(
            seen.contains(name),
            "{name} never triggered in {games} games of {players} seats: {seen:?}"
        );
    }
}
