//! Goal: a player who can't attack (`Prohibited::R118Attack`) declares no attack this turn, one who can't attack "you" still attacks the other players, a ban for "their next turn" waits for that turn and ends with it, and "haven't begun an extra turn this game" (`Condition::R118BegunExtraTurn`) holds until the player's first extra turn begins; at tables of two, three and four seats, each log replaying to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::modify::{Duration, Prohibited};
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The table with seat 1 to move in its main phase, a Bear ready to attack and a Watcher of seat 0 that makes the rules.
fn r118_table(seats: usize, turn: u32, active: usize) -> Scenario {
    let scenario = (0..seats).fold(Scenario::new(seats), |scenario, seat| {
        scenario.library(seat, (0..12).map(|_| forest()))
    });
    scenario
        .at(turn, active, Step::Main1)
        .battlefield(0, [ready(free_creature("Watcher", 0, 4))])
        .battlefield(1, [ready(free_creature("Bear", 2, 2))])
}

/// The ban on attacking seat 1 gets from seat 0's Watcher.
fn r118_ban(driver: &Driver, you: bool) -> Rule {
    Rule::Cant {
        who: vec![driver.target_player(1)],
        action: Box::new(Prohibited::R118Attack { you, walkers: false }),
    }
}

/// Whether seat 1's Bear may attack the seat `defender` now; the log replays to the live state either way.
fn r118_attacks(driver: &mut Driver, defender: usize) -> bool {
    let allowed = driver.pass_until(Step::DeclareAttackers).is_ok()
        && driver.attack(&[("Bear", defender)]).is_ok();
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    allowed
}

#[rstest]
fn r118_a_player_who_cant_attack_attacks_no_one(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] you: bool,
    #[values(0, 1)] last: usize,
) -> Result<(), ScriptError> {
    // The defender is seat 0, who made the rule, or the last seat.
    let defender = if last == 0 { 0 } else { seats - 1 };
    if defender == 1 {
        return Ok(());
    }
    let mut driver = Driver::from_scenario(r118_table(seats, 2, 1))?;
    let watcher = driver.id_of("Watcher")?;
    let ban = r118_ban(&driver, you);
    driver.add_rule_from(watcher, ban, Duration::UntilEndOfTurn)?;
    // "can't attack you" leaves every other player open; "can't attack" leaves no one.
    let open = you && defender != 0;
    assert_eq!(r118_attacks(&mut driver, defender), open);
    Ok(())
}

#[rstest]
fn r118_a_ban_for_their_next_turn_waits_for_it_and_ends_with_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] later: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r118_table(seats, 2, 0))?;
    let watcher = driver.id_of("Watcher")?;
    let ban = r118_ban(&driver, true);
    driver.add_rule_from(watcher, ban, Duration::TheirNextTurn)?;
    // Seat 1's next turn is the next one; their turn after that comes a round later.
    let turn = if later {
        3 + u32::try_from(seats).unwrap()
    } else {
        3
    };
    driver.idle_until(turn, Step::Main1)?;
    assert_eq!(r118_attacks(&mut driver, 0), later);
    Ok(())
}

/// "If you haven't begun an extra turn this game, take an extra turn after this one."
const R118_ONCE: &str = r#"{"Spell":{"effects":[{"If":{"condition":{"Not":{"R118BegunExtraTurn":"You"}},"then":[{"ExtraTurn":{}}]}}]}}"#;

#[rstest]
fn r118_an_extra_turn_begun_is_remembered_for_the_game(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let ability: Ability = serde_json::from_str(R118_ONCE).unwrap();
    let once = |name: &str| {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Sorcery)
                .with_mana_cost(cost(0, &[]))
                .with_rules(ability.clone()),
        )
    };
    let scenario = (0..seats).fold(Scenario::new(seats), |scenario, seat| {
        scenario.library(seat, (0..12).map(|_| forest()))
    });
    let scenario = scenario
        .at(2, 0, Step::Main1)
        .hand(0, [once("First Time"), once("Second Time")]);
    let mut driver = Driver::from_scenario(scenario)?;
    let me = driver.state().turn().active();
    driver.cast("First Time")?;
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(
        driver.state().turn().active(),
        me,
        "the extra turn is taken"
    );
    assert!(driver.state().turn().is_extra());
    driver.cast("Second Time")?;
    driver.idle_until(4, Step::Main1)?;
    assert_ne!(driver.state().turn().active(), me, "no second extra turn");
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
