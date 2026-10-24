//! Goal: a result bound to the tokens an effect created (counters on each, haste until end of turn, an Equipment attached, a must-attack rule) reaches every token made and none when none were made, at tables of two, three and four seats, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;
use mtg_engine::keywords::has_keyword;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup, characteristics, replay};
use mtg_testkit::cards::{bear, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A permanent of `kind` that makes what `effects` say when it enters.
fn maker(kind: CardType, effects: &str) -> Arc<CardDefinition> {
    let trigger = format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":{effects}}}}}"#
    );
    let mut card = CardDefinition::new("Maker")
        .with_type(kind)
        .with_subtype(if kind == CardType::Artifact {
            "Equipment"
        } else {
            "Maker"
        })
        .with_mana_cost(ManaCost::parse("{1}").unwrap())
        .with_rules(serde_json::from_str::<Ability>(&trigger).unwrap());
    if kind == CardType::Creature {
        card = card.with_power_toughness(1, 1);
    }
    Arc::new(card)
}

/// Seat 0 holds the Maker and three Mountains; every other seat has a Bear.
fn table(seats: usize, kind: CardType, effects: &str) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [maker(kind, effects)])
        .battlefield(0, [ready(mountain()), ready(mountain()), ready(mountain())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    scenario
}

fn run(seats: usize, kind: CardType, effects: &str) -> Result<(Driver, GameSetup), ScriptError> {
    let setup = table(seats, kind, effects).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, kind, effects))?;
    driver.cast_tapping("Maker", &["Mountain"])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok((driver, setup))
}

fn assert_replays(driver: &Driver, setup: &GameSetup) {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
}

/// The tokens on the battlefield that seat 0 controls.
fn tokens(driver: &Driver) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.kind() == ObjectKind::Token && o.controller() == pid(0))
        })
        .collect()
}

const SNAKE: &str = r#"{"Custom":{"name":"Snake","colors":["Green"],"types":["Creature"],"subtypes":["Snake"],"power":1,"toughness":1}}"#;

fn made(count: u32) -> String {
    format!(r#"{{"CreateToken":{{"token":{SNAKE},"count":{count},"as":"tokens"}}}}"#)
}

#[rstest]
fn counters_land_on_each_token_made_and_on_none_when_none_were_made(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] count: u32,
) -> Result<(), ScriptError> {
    let put = r#"{"PutCounters":{"on":{"Bound":"tokens"},"counter":"+1/+1","count":2}}"#;
    let (driver, setup) = run(
        seats,
        CardType::Creature,
        &format!("[{},{put}]", made(count)),
    )?;
    let found = tokens(&driver);
    assert_eq!(found.len(), count as usize);
    for id in found {
        let state = driver.state();
        assert_eq!(
            state
                .object(id)
                .unwrap()
                .permanent()
                .unwrap()
                .counters_of(&CounterKind::PlusOnePlusOne),
            2
        );
        let chars = characteristics(state, id).unwrap();
        assert_eq!((chars.power, chars.toughness), (Some(3), Some(3)));
    }
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_tokens_made_gain_haste_and_nobody_else_does(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] count: u32,
) -> Result<(), ScriptError> {
    let haste = r#"{"Continuous":{"affected":{"Bound":"tokens"},"mods":[{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}"#;
    let (driver, setup) = run(
        seats,
        CardType::Creature,
        &format!("[{},{haste}]", made(count)),
    )?;
    let found = tokens(&driver);
    assert_eq!(found.len(), count as usize);
    for id in &found {
        assert!(has_keyword(driver.state(), *id, &Keyword::Haste));
    }
    let bears = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| !found.contains(id))
        .filter(|id| has_keyword(driver.state(), **id, &Keyword::Haste))
        .count();
    assert_eq!(bears, 0, "only the new tokens gain haste");
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn an_equipment_attaches_to_the_token_it_made(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let attach = r#"{"Attach":{"what":"This","to":{"Bound":"tokens"}}}"#;
    let (driver, setup) = run(
        seats,
        CardType::Artifact,
        &format!("[{},{attach}]", made(1)),
    )?;
    let [token] = tokens(&driver)[..] else {
        panic!("one token");
    };
    let state = driver.state();
    let equipment = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == "Maker"))
        .expect("the Equipment");
    assert_eq!(state.host_of(equipment), Some(token));
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_must_attack_rule_over_the_tokens_made_leaves_the_game_playable(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] count: u32,
) -> Result<(), ScriptError> {
    let rule = r#"{"Rule":{"rule":{"MustAttack":{"Bound":"tokens"}},"duration":"UntilEndOfTurn"}}"#;
    let (driver, setup) = run(
        seats,
        CardType::Creature,
        &format!("[{},{rule}]", made(count)),
    )?;
    assert_eq!(tokens(&driver).len(), count as usize);
    assert_replays(&driver, &setup);
    Ok(())
}
