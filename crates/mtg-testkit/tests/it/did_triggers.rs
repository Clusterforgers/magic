//! Goal: `Did { action, who, what }` plays whole at tables of two, three and four seats: a permanent that waits for an action (a land played, an ability activated, a scry, a shuffle, a destroy, an exile, a surveil, a proliferate, a loss of the game, a change of controller) triggers once for the action, only for the players and objects the pattern names, never for the same action of someone else, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::{GameSetup, PresetPermanent, replay};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{pid, spell};

const LIFE: i32 = 10;

/// A creature that gains its controller `LIFE` life whenever `did`.
fn watcher(did: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{{"Did":{did}}},"effects":[{{"GainLife":{{"amount":{LIFE}}}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rule),
    )
}

/// A creature with an ability that shuffles its controller's library for no cost.
fn shuffler() -> Arc<CardDefinition> {
    let rule: Ability =
        serde_json::from_str(r#"{"Activated":{"cost":[],"effects":[{"Shuffle":{}}]}}"#).unwrap();
    Arc::new(
        CardDefinition::new("Shuffler")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rule),
    )
}

/// The table: seat 0 holds `hand`; the watcher of `did` is under `watcher_seat`'s control, a Victim and a Shuffler under
/// seat 1's, everyone has a library.
fn table(seats: usize, did: &str, watcher_seat: usize, hand: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut scenario = Scenario::new(seats).hand(0, hand);
    for seat in 0..seats {
        let mut field: Vec<PresetPermanent> = Vec::new();
        if seat == watcher_seat {
            field.push(ready(watcher(did)));
        }
        if seat == 1 {
            field.push(ready(free_creature("Victim", 2, 2)));
            field.push(ready(shuffler()));
        }
        scenario = scenario
            .library(seat, vec![forest(); 12])
            .battlefield(seat, field);
    }
    scenario
}

/// Answers whatever is asked passively until nothing is on the stack and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

const TARGET_CREATURE: &str = r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#;

#[rstest]
#[case::scry_by_me("Scry", r#"[{"Scry":{}}]"#, "[]", r#"{"action":"Scried"}"#, 0, 1)]
#[case::scry_by_me_for_me(
    "Scry",
    r#"[{"Scry":{}}]"#,
    "[]",
    r#"{"action":"Scried","who":"You"}"#,
    0,
    1
)]
#[case::scry_by_me_not_for_an_opponent(
    "Scry",
    r#"[{"Scry":{}}]"#,
    "[]",
    r#"{"action":"Scried","who":"You"}"#,
    1,
    0
)]
#[case::scry_by_an_opponent_of_the_watcher(
    "Scry",
    r#"[{"Scry":{}}]"#,
    "[]",
    r#"{"action":"Scried","who":"Opponents"}"#,
    1,
    1
)]
#[case::surveil(
    "Surveil",
    r#"[{"Surveil":{}}]"#,
    "[]",
    r#"{"action":"Surveilled"}"#,
    1,
    1
)]
#[case::shuffle(
    "Shuffle",
    r#"[{"Shuffle":{}}]"#,
    "[]",
    r#"{"action":"Shuffled"}"#,
    0,
    1
)]
#[case::an_action_nobody_took(
    "Shuffle",
    r#"[{"Shuffle":{}}]"#,
    "[]",
    r#"{"action":"Scried"}"#,
    0,
    0
)]
#[case::proliferate(
    "Proliferate",
    r#"[{"Proliferate":{}}]"#,
    "[]",
    r#"{"action":"Proliferated"}"#,
    0,
    1
)]
#[case::destroy_a_creature(
    "Destroy",
    r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    TARGET_CREATURE,
    r#"{"action":"Destroyed","what":[{"Type":"Creature"}]}"#,
    0,
    1
)]
#[case::destroy_not_a_land(
    "Destroy",
    r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    TARGET_CREATURE,
    r#"{"action":"Destroyed","what":[{"Type":"Land"}]}"#,
    0,
    0
)]
#[case::destroy_what_an_opponent_controls(
    "Destroy",
    r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    TARGET_CREATURE,
    r#"{"action":"Destroyed","who":"Opponents"}"#,
    0,
    1
)]
#[case::destroy_what_i_control_not_theirs(
    "Destroy",
    r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    TARGET_CREATURE,
    r#"{"action":"Destroyed","who":"You"}"#,
    0,
    0
)]
#[case::exile(
    "Exile",
    r#"[{"Exile":{"what":{"Target":0}}}]"#,
    TARGET_CREATURE,
    r#"{"action":"Exiled","what":[{"Type":"Creature"}]}"#,
    0,
    1
)]
#[case::a_destroy_is_not_an_exile(
    "Destroy",
    r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    TARGET_CREATURE,
    r#"{"action":"Exiled"}"#,
    0,
    0
)]
#[case::control_changes_to_me(
    "Threaten",
    r#"[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}}]"#,
    TARGET_CREATURE,
    r#"{"action":"ChangedController","who":"You","what":[{"Type":"Creature"}]}"#,
    0,
    1
)]
#[case::control_changes_to_me_not_for_the_other_side(
    "Threaten",
    r#"[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}}]"#,
    TARGET_CREATURE,
    r#"{"action":"ChangedController","who":"Opponents"}"#,
    0,
    0
)]
fn a_spell_that_does_something_triggers_what_waits_for_the_action_it_names(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] effects: &str,
    #[case] targets: &str,
    #[case] did: &str,
    #[case] watcher_seat: usize,
    #[case] times: i32,
) -> Result<(), ScriptError> {
    let scenario = || {
        table(
            seats,
            did,
            watcher_seat,
            vec![spell(name, CardType::Instant, targets, effects)],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let before = life(&driver, watcher_seat);
    if targets == "[]" {
        driver.cast(name)?;
    } else {
        let target = driver.target_named("Victim")?;
        driver.cast_at(name, &[target])?;
    }
    settle(&mut driver)?;
    assert_eq!(life(&driver, watcher_seat), before + times * LIFE);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::any_land(r#"{"action":"PlayedLand"}"#, 1)]
#[case::my_land(r#"{"action":"PlayedLand","who":"You"}"#, 1)]
#[case::their_land(r#"{"action":"PlayedLand","who":"Opponents"}"#, 0)]
#[case::a_forest(r#"{"action":"PlayedLand","what":[{"Type":"Land"}]}"#, 1)]
#[case::not_a_creature(r#"{"action":"PlayedLand","what":[{"Type":"Creature"}]}"#, 0)]
fn a_land_played_triggers_what_waits_for_it_and_only_for_the_land_and_player_it_names(
    #[values(2, 3, 4)] seats: usize,
    #[case] did: &str,
    #[case] times: i32,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, did, 0, vec![forest()]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let before = life(&driver, 0);
    driver.play_land("Forest")?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), before + times * LIFE);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::concede_of_an_opponent(r#"{"action":"LostGame","who":"Opponents"}"#, 1, 1)]
#[case::anyone_losing(r#"{"action":"LostGame"}"#, 1, 1)]
#[case::i_lose_the_game_not_them(r#"{"action":"LostGame","who":"You"}"#, 1, 0)]
fn a_player_who_loses_the_game_triggers_what_waits_for_it(
    #[values(3, 4)] seats: usize,
    #[case] did: &str,
    #[case] loser: usize,
    #[case] times: i32,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, did, 0, Vec::new());
    let mut driver = Driver::from_scenario(scenario())?;
    let before = life(&driver, 0);
    driver.concede(pid(loser))?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), before + times * LIFE);
    Ok(())
}

#[rstest]
#[case::activated(r#"{"action":"Activated"}"#, 0, 1)]
#[case::activated_by_an_opponent(r#"{"action":"Activated","who":"Opponents"}"#, 0, 1)]
#[case::not_by_me(r#"{"action":"Activated","who":"You"}"#, 0, 0)]
#[case::of_a_creature(r#"{"action":"Activated","what":[{"Type":"Creature"}]}"#, 0, 1)]
#[case::of_a_land(r#"{"action":"Activated","what":[{"Type":"Land"}]}"#, 0, 0)]
fn an_ability_activated_triggers_what_waits_for_it_for_the_player_and_source_it_names(
    #[values(2, 3, 4)] seats: usize,
    #[case] did: &str,
    #[case] watcher_seat: usize,
    #[case] times: i32,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, did, watcher_seat, Vec::new());
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    // Seat 0 holds priority; its opponent's ability is put on the stack by passing to it.
    crate::common::removal::pass_to(&mut driver, 1)?;
    let before = life(&driver, watcher_seat);
    driver.begin_ability("Shuffler", 0)?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, watcher_seat), before + times * LIFE);
    replays(&driver, &setup);
    Ok(())
}

/// A creature named Chain that gains its controller 3 life whenever an ability of a Watcher triggers.
fn chain() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"Did":{"action":"AbilityTriggered","what":[{"Named":"Watcher"}]}},"effects":[{"GainLife":{"amount":3}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Chain")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rule),
    )
}

#[rstest]
fn an_ability_that_triggers_triggers_what_waits_for_that(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, vec![forest()])
            .library(0, vec![forest(); 12])
            .battlefield(
                0,
                vec![
                    ready(watcher(r#"{"action":"PlayedLand","who":"You"}"#)),
                    ready(chain()),
                ],
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let before = life(&driver, 0);
    driver.play_land("Forest")?;
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(life(&driver, 0), before + LIFE + 3);
    replays(&driver, &setup);
    Ok(())
}
