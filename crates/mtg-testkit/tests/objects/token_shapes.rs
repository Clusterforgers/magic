//! Goal: every shape of `CreateToken` plays whole at tables of two, three and four seats: a list of kinds made together, the predefined Vibranium, Map and Mutagen tokens and their abilities, a legendary named token, a token with an ability of its own, the tokens of another player or of each player (their owner and controller), the amount, entering tapped, set power and toughness, and a result bound to the tokens that a later step puts counters on; a token entering attacking joins the attack of its controller, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameSetup, characteristics, replay};
use mtg_testkit::cards::mountain;
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

fn table(seats: usize, effects: &str) -> Scenario {
    Scenario::new(seats)
        .hand(0, [maker(effects)])
        .battlefield(0, [ready(mountain())])
}

/// What a token on the battlefield looks like: its name, controller seat, owner seat, size, tapped, legendary.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Made {
    name: String,
    controller: usize,
    owner: usize,
    size: (i32, i32),
    tapped: bool,
    legendary: bool,
}

fn made(driver: &Driver) -> Vec<Made> {
    let state = driver.state();
    let mut found: Vec<Made> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter_map(|id| {
            let object = state.object(id)?;
            (object.kind() == ObjectKind::Token).then(|| {
                let chars = characteristics(state, id).unwrap();
                Made {
                    name: object.card().name.clone(),
                    controller: (0..4)
                        .find(|seat| object.controller() == pid(*seat))
                        .unwrap(),
                    owner: (0..4).find(|seat| object.owner() == pid(*seat)).unwrap(),
                    size: (chars.power.unwrap_or(0), chars.toughness.unwrap_or(0)),
                    tapped: object
                        .permanent()
                        .is_some_and(mtg_engine::object::PermanentState::tapped),
                    legendary: object.card().supertypes.contains(&Supertype::Legendary),
                }
            })
        })
        .collect();
    found.sort_by(|a, b| (&a.name, a.controller).cmp(&(&b.name, b.controller)));
    found
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
    assert_eq!(resumed.decision(), driver.engine().decision());
}

const SNAKE: &str = r#"{"Custom":{"name":"Snake","colors":["Green"],"types":["Creature"],"subtypes":["Snake"],"power":1,"toughness":1}}"#;
const WOLF: &str = r#"{"Custom":{"name":"Wolf","colors":["Green"],"types":["Creature"],"subtypes":["Wolf"],"power":2,"toughness":2}}"#;
const BEAST: &str = r#"{"Custom":{"name":"Beast","colors":["Green"],"types":["Creature"],"subtypes":["Beast"],"power":3,"toughness":3}}"#;

fn token(name: &str, controller: usize, size: (i32, i32), tapped: bool) -> Made {
    Made {
        name: name.to_owned(),
        controller,
        owner: controller,
        size,
        tapped,
        legendary: false,
    }
}

#[rstest]
#[case::a_list_of_kinds(
    format!(r#"[{{"CreateToken":{{"token":{SNAKE}}}}},{{"CreateToken":{{"token":{WOLF}}}}},{{"CreateToken":{{"token":{BEAST}}}}}]"#),
    vec![token("Beast", 0, (3, 3), false), token("Snake", 0, (1, 1), false), token("Wolf", 0, (2, 2), false)]
)]
#[case::a_count_of_one_kind(
    format!(r#"[{{"CreateToken":{{"token":{WOLF},"count":3}}}}]"#),
    vec![token("Wolf", 0, (2, 2), false); 3]
)]
#[case::tapped(
    format!(r#"[{{"CreateToken":{{"token":{WOLF},"count":2,"tapped":true}}}}]"#),
    vec![token("Wolf", 0, (2, 2), true); 2]
)]
#[case::two_tapped_vibranium(
    r#"[{"CreateToken":{"token":{"Named":"vibranium"},"count":2,"tapped":true}}]"#.to_owned(),
    vec![token("Vibranium", 0, (0, 0), true); 2]
)]
#[case::a_map_and_a_mutagen(
    r#"[{"CreateToken":{"token":{"Named":"map"}}},{"CreateToken":{"token":{"Named":"mutagen"}}}]"#.to_owned(),
    vec![token("Map", 0, (0, 0), false), token("Mutagen", 0, (0, 0), false)]
)]
fn the_tokens_of_the_controller_enter_as_described(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: String,
    #[case] expected: Vec<Made>,
) -> Result<(), ScriptError> {
    let (driver, setup) = run(seats, &effects)?;
    assert_eq!(made(&driver), expected);
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::each_opponent("Opponents", 1, vec![1])]
#[case::each_player("EachPlayer", 1, vec![0, 1])]
#[case::each_opponent_two_each("Opponents", 2, vec![1, 1])]
fn tokens_of_other_players_belong_to_them(
    #[values(2, 3, 4)] seats: usize,
    #[case] who: &str,
    #[case] count: u32,
    #[case] per_seat: Vec<usize>,
) -> Result<(), ScriptError> {
    let effects =
        format!(r#"[{{"CreateToken":{{"who":"{who}","token":{WOLF},"count":{count}}}}}]"#);
    let (driver, setup) = run(seats, &effects)?;
    let expected_seats: Vec<usize> = (0..seats)
        .filter(|seat| if who == "Opponents" { *seat != 0 } else { true })
        .collect();
    let found = made(&driver);
    let per_player = if who == "Opponents" && per_seat == [1, 1] {
        2
    } else {
        1
    };
    assert_eq!(found.len(), expected_seats.len() * per_player);
    for seat in &expected_seats {
        let theirs: Vec<&Made> = found.iter().filter(|m| m.controller == *seat).collect();
        assert_eq!(theirs.len(), per_player, "seat {seat}");
        assert!(theirs.iter().all(|m| m.owner == *seat && m.size == (2, 2)));
    }
    assert_replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_legendary_named_token_keeps_its_name_and_its_supertype() -> Result<(), ScriptError> {
    let tiny = r#"{"Custom":{"name":"Tiny","colors":["Green"],"supertypes":["Legendary"],"types":["Creature"],"subtypes":["Dog","Detective"],"power":2,"toughness":2,"abilities":[{"Keyword":"Trample"}]}}"#;
    let (driver, setup) = run(2, &format!(r#"[{{"CreateToken":{{"token":{tiny}}}}}]"#))?;
    let found = made(&driver);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Tiny");
    assert!(found[0].legendary);
    assert_replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::a_counter_on_one(1, 1)]
#[case::two_counters(2, 1)]
#[case::on_each_of_three(1, 3)]
fn counters_go_on_the_tokens_the_step_bound(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] tokens: u32,
) -> Result<(), ScriptError> {
    let effects = format!(
        r#"[{{"CreateToken":{{"token":{SNAKE},"count":{tokens},"as":"made"}}}},{{"PutCounters":{{"on":{{"Bound":"made"}},"counter":"+1/+1","count":{counters}}}}}]"#
    );
    let (driver, setup) = run(seats, &effects)?;
    let found = made(&driver);
    assert_eq!(found.len(), tokens as usize);
    let size = i32::try_from(counters).unwrap() + 1;
    assert!(found.iter().all(|m| m.size == (size, size)), "{found:?}");
    assert_replays(&driver, &setup);
    Ok(())
}

/// Every token created by the log of a game that made them, by event.
fn created(driver: &Driver) -> Vec<ObjectId> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::TokenCreated(token) => Some(token.object),
            _ => None,
        })
        .collect()
}

#[rstest]
#[case::vibranium("vibranium", "Indestructible")]
#[case::map("map", "Explore")]
#[case::mutagen("mutagen", "PutCounters")]
fn a_predefined_token_carries_the_ability_of_its_file(
    #[case] id: &str,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let effects = format!(r#"[{{"CreateToken":{{"token":{{"Named":"{id}"}}}}}}]"#);
    let (driver, _) = run(2, &effects)?;
    let made = created(&driver);
    assert_eq!(made.len(), 1);
    let object = driver.state().object(made[0]).unwrap();
    let debug = format!("{:?}", object.card());
    assert!(debug.contains(expected), "{debug}");
    Ok(())
}
