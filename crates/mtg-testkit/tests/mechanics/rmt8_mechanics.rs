//! Goal: pack rmt8's mechanics play whole at tables of two, three and four seats: a creature evolves only when its own evolve ability puts counters on it, so "whenever this creature evolves" triggers once for each evolve and never for counters another ability puts; a harnessed permanent's ∞ ability works and an unharnessed one's does not; a spell its owner can't cast during their first three turns is cast from their fourth; another player who loses the game gives the life total they had as the turn began (at three and four seats, so the game goes on); and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::{free_creature, keyworded};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn with(card: &Arc<CardDefinition>, rules: &[&str]) -> Arc<CardDefinition> {
    let mut card = (**card).clone();
    for rule in rules {
        let ability: Ability = serde_json::from_str(rule).unwrap();
        card = card.with_rules(ability);
    }
    Arc::new(card)
}

/// Passes and answers the first legal target until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::ChooseTargets(options) => {
                let first = options.legal[0];
                driver.choose_targets(&[first])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn assert_replays(driver: &Driver, setup: &GameSetup) {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
}

/// Evolve, "Whenever this creature evolves, you gain 1 life." and "{0}: Put a +1/+1 counter on this creature."
fn evolver() -> Arc<CardDefinition> {
    with(
        &keyworded("Evo", 1, 1, &[Keyword::Evolve]),
        &[
            r#"{"Triggered":{"when":{"Did":{"action":"Evolved","what":["This"]}},"effects":[{"GainLife":{"amount":1}}]}}"#,
            r#"{"Activated":{"cost":"{0}","effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
        ],
    )
}

#[rstest]
#[case::bigger(&[(2, 1)], 21)]
#[case::not_bigger(&[(1, 1)], 20)]
#[case::twice(&[(2, 1), (3, 3)], 22)]
fn a_creature_evolves_when_its_evolve_ability_puts_a_counter_on_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] entering: &[(i32, i32)],
    #[case] expected_life: i32,
) -> Result<(), ScriptError> {
    let table = || {
        let hand: Vec<_> = entering
            .iter()
            .enumerate()
            .map(|(at, (power, toughness))| {
                free_creature(&format!("Entrant{at}"), *power, *toughness)
            })
            .collect();
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(evolver())])
            .hand(0, hand)
    };
    let mut driver = Driver::from_scenario(table())?;
    for at in 0..entering.len() {
        driver.cast(&format!("Entrant{at}"))?;
        settle(&mut driver)?;
    }
    assert_eq!(life(&driver, 0), expected_life);
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

#[rstest]
fn a_counter_from_another_ability_is_no_evolve(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(evolver())])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.begin_ability("Evo", 2)?;
    settle(&mut driver)?;
    let evo = driver.id_of("Evo")?;
    let counters = driver
        .state()
        .object(evo)
        .and_then(|object| object.permanent())
        .map_or(0, |held| {
            held.counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne)
        });
    assert_eq!(counters, 1);
    assert_eq!(life(&driver, 0), 20);
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

/// "{0}: Harness this artifact." and "∞ — At the beginning of your upkeep, you gain 1 life."
fn stone() -> Arc<CardDefinition> {
    with(
        &Arc::new(CardDefinition::new("Stone").with_type(CardType::Artifact)),
        &[
            r#"{"Activated":{"cost":"{0}","effects":[{"Harness":{"what":"This"}}]}}"#,
            r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Matches":{"subject":"This","selector":["Harnessed"]}},"effects":[{"GainLife":{"amount":1}}]}}"#,
        ],
    )
}

#[rstest]
#[case::harnessed(true, 21)]
#[case::not_harnessed(false, 20)]
fn the_infinity_ability_works_once_its_permanent_is_harnessed(
    #[values(2, 3, 4)] seats: usize,
    #[case] harness: bool,
    #[case] expected_life: i32,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(stone())])
    };
    let mut driver = Driver::from_scenario(table())?;
    if harness {
        driver.begin_ability("Stone", 0)?;
        settle(&mut driver)?;
        // Harnessing it again does nothing more: it stays harnessed.
        driver.begin_ability("Stone", 0)?;
        settle(&mut driver)?;
    }
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Draw)?;
    assert_eq!(life(&driver, 0), expected_life);
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

/// "You can't cast this spell during your first, second, or third turns of the game."
fn latecomer() -> Arc<CardDefinition> {
    with(
        &free_creature("Latecomer", 2, 2),
        &[
            r#"{"CastOnly":{"condition":{"Not":{"All":["YourTurn",{"Compare":{"left":{"TurnsTaken":"You"},"op":"Le","right":3}}]}}}}"#,
        ],
    )
}

#[rstest]
#[case::first_turn(0, false)]
#[case::third_turn(2, false)]
#[case::fourth_turn(3, true)]
fn a_spell_is_cast_only_from_its_owners_fourth_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] rounds: u32,
    #[case] castable: bool,
) -> Result<(), ScriptError> {
    let turn = rounds * u32::try_from(seats).unwrap() + 1;
    let table = || {
        Scenario::new(seats)
            .at(turn, 0, Step::Main1)
            .hand(0, [latecomer()])
    };
    let mut driver = Driver::from_scenario(table())?;
    assert_eq!(driver.cast("Latecomer").is_ok(), castable);
    Ok(())
}

/// "Whenever another player loses the game, you gain life equal to that player's life total as the turn began."
fn baron() -> Arc<CardDefinition> {
    with(
        &free_creature("Baron", 2, 2),
        &[
            r#"{"Triggered":{"when":{"Did":{"action":"LostGame","who":{"PlayersWhere":{"who":"EachPlayer","test":{"Not":{"Among":"Asker"}}}}}},"effects":[{"GainLife":{"amount":{"LifeAsTurnBegan":"TriggeringPlayer"}}}]}}"#,
        ],
    )
}

#[rstest]
fn another_player_who_loses_gives_their_life_as_the_turn_began(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(baron())])
            .battlefield(1, [ready(baron())])
            .life(2, 7)
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.concede(pid(2))?;
    settle(&mut driver)?;
    // Each Baron's controller gains the 7 life the loser had as the turn began.
    assert_eq!(life(&driver, 0), 27);
    assert_eq!(life(&driver, 1), 27);
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}
