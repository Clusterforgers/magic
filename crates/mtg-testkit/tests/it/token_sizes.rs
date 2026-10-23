//! Goal: an X/X token whose size is an amount enters at the size the amount has when it is created (nothing, one and many; a count of permanents, a life total, the creatures that died this turn for every player or for one), a result bound to it adds counters, the static ability and the abilities in quotes that a token has stay with it, and the log replays to the live state at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameSetup, characteristics, replay};
use mtg_testkit::cards::{bear, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A creature that makes what `effects` say when it enters.
fn maker(effects: &str) -> Arc<CardDefinition> {
    let trigger = format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":{effects}}}}}"#
    );
    Arc::new(
        CardDefinition::new("Maker")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse("{1}").unwrap())
            .with_rules(serde_json::from_str::<Ability>(&trigger).unwrap()),
    )
}

/// Seat 0 holds the Maker and three Mountains; every other seat has a Bear.
fn table(seats: usize, effects: &str) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [maker(effects)])
        .battlefield(0, [ready(mountain()), ready(mountain()), ready(mountain())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    scenario
}

fn run(seats: usize, effects: &str) -> Result<(Driver, GameSetup), ScriptError> {
    let setup = table(seats, effects).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, effects))?;
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

/// The sizes of the tokens on the battlefield, of seat 0.
fn sizes(driver: &Driver) -> Vec<(i32, i32)> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter_map(|id| {
            let object = state.object(id)?;
            (object.kind() == ObjectKind::Token && object.controller() == pid(0)).then(|| {
                let chars = characteristics(state, id).unwrap();
                (chars.power.unwrap_or(0), chars.toughness.unwrap_or(0))
            })
        })
        .collect()
}

/// How many tokens the log created.
fn created(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::TokenCreated(_)))
        .count()
}

fn sized(size: &str, extra: &str) -> String {
    sized_as(size, extra, "")
}

fn sized_as(size: &str, extra: &str, bind: &str) -> String {
    format!(
        r#"{{"CreateToken":{{"token":{{"Sized":{{"size":{size},"token":{{"name":"Horror","colors":["Black"],"types":["Creature"],"subtypes":["Horror"]{extra}}}}}}}{bind}}}}}"#
    )
}

#[rstest]
#[case::one("1", vec![(1, 1)])]
#[case::a_count_of_lands(r#"{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}"#, vec![(3, 3)])]
#[case::a_life_total(r#"{"Life":"You"}"#, vec![(20, 20)])]
#[case::twice_a_count(
    r#"{"Times":{"by":2,"of":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}"#,
    vec![(6, 6)]
)]
fn the_token_has_the_size_the_amount_has_when_it_is_created(
    #[values(2, 3, 4)] seats: usize,
    #[case] size: &str,
    #[case] expected: Vec<(i32, i32)>,
) -> Result<(), ScriptError> {
    let (driver, setup) = run(seats, &format!("[{}]", sized(size, "")))?;
    assert_eq!(sizes(&driver), expected);
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_size_of_nothing_is_a_token_that_does_not_stay(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, setup) = run(seats, &format!("[{}]", sized("0", "")))?;
    assert_eq!(created(&driver), 1, "the token was created");
    assert!(
        sizes(&driver).is_empty(),
        "a 0/0 token has no toughness left"
    );
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::every_player(
    r#"{"Tallied":{"who":"EachPlayer","what":"CreaturesDied"}}"#,
    |seats: usize| i32::try_from(seats - 1).unwrap()
)]
#[case::the_creatures_of_seat_one(
    r#"{"Tallied":{"who":{"Each":[{"Type":"Creature"}]},"what":"CreaturesDied"}}"#,
    |_| 0
)]
fn the_creatures_that_died_this_turn_size_the_token(
    #[values(2, 3, 4)] seats: usize,
    #[case] size: &str,
    #[case] expected: fn(usize) -> i32,
) -> Result<(), ScriptError> {
    // Each opponent sacrifices their Bear, then the token is made: one creature died for each of them.
    let effects = format!(
        r#"[{{"Sacrifice":{{"who":"Opponents","filter":[{{"Type":"Creature"}}]}}}},{}]"#,
        sized(size, "")
    );
    let (driver, setup) = run(seats, &effects)?;
    let size = expected(seats);
    let found = sizes(&driver);
    if size == 0 {
        assert!(found.is_empty(), "{found:?}");
    } else {
        assert_eq!(found, vec![(size, size)]);
    }
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_result_bound_to_a_sized_token_adds_to_its_size(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] counters: u32,
) -> Result<(), ScriptError> {
    let make = sized_as("2", "", r#","as":"made""#);
    let put = format!(
        r#"{{"PutCounters":{{"on":{{"Bound":"made"}},"counter":"+1/+1","count":{counters}}}}}"#
    );
    let (driver, setup) = run(seats, &format!("[{make},{put}]"))?;
    let size = 2 + i32::try_from(counters).unwrap();
    assert_eq!(sizes(&driver), vec![(size, size)]);
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::a_static_in_quotes(
    r#","abilities":[{"Static":{"affected":"This","mods":["CantBlock"]}}]"#,
    "CantBlock"
)]
#[case::an_ability_in_quotes(
    r#","abilities":[{"Activated":{"cost":["SacrificeSelf"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}]"#,
    "AddMana"
)]
fn the_abilities_a_sized_token_is_given_stay_with_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] extra: &str,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let (driver, setup) = run(seats, &format!("[{}]", sized("2", extra)))?;
    let state = driver.state();
    let token = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.kind() == ObjectKind::Token)
        })
        .expect("the token");
    let debug = format!("{:?}", state.object(token).unwrap().card());
    assert!(debug.contains(expected), "{debug}");
    assert_replays(&driver, &setup);
    Ok(())
}
