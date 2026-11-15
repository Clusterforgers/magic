//! Goal: the dice a player rolled this turn are kept by result (CR 706.2), at 2 to 4 seats: Eelectrocute may be cast from its owner's graveyard exactly when they rolled a 6 this turn on any die (its ruling: a d20 counts), it then deals its damage and is exiled instead of going to the graveyard, and the game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{cost, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An instant for nothing that rolls a d20 and does nothing else.
fn roller() -> Arc<CardDefinition> {
    let rules: Ability =
        serde_json::from_str(r#"{"Spell":{"effects":[{"RollDice":{"sides":20,"results":[]}}]}}"#)
            .unwrap();
    Arc::new(
        CardDefinition::new("Roller")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

fn table(seats: usize, seed: u64) -> Scenario {
    Scenario::new(seats)
        .seed(seed)
        .hand(0, vec![roller()])
        .graveyard_named(0, ["Eelectrocute"])
        .battlefield(0, [ready(mountain()), ready(mountain())])
}

/// Passes priority and answers passively until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(());
        };
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn castable(driver: &Driver, name: &str) -> bool {
    let Some(DecisionKind::Priority(options)) = driver
        .engine()
        .decision()
        .map(|decision| decision.kind.clone())
    else {
        return false;
    };
    options
        .castable
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == name)
}

fn total_life(driver: &Driver) -> i32 {
    driver
        .state()
        .players()
        .iter()
        .map(mtg_engine::player::Player::life)
        .sum()
}

#[rstest]
fn eelectrocute_is_cast_from_the_graveyard_once_a_6_was_rolled(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut sixes = 0;
    for seed in 0..60 {
        let mut driver = Driver::from_scenario(table(seats, seed))?;
        assert!(!castable(&driver, "Eelectrocute"), "nothing rolled yet");
        driver.cast("Roller")?;
        settle(&mut driver)?;
        let rolled: Vec<u32> = driver
            .engine()
            .events()
            .iter()
            .filter_map(|event| match event {
                Event::DieRolled { result, .. } => Some(*result),
                _ => None,
            })
            .collect();
        assert_eq!(rolled.len(), 1);
        let six = rolled[0] == 6;
        assert_eq!(
            castable(&driver, "Eelectrocute"),
            six,
            "seed {seed}: rolled {rolled:?}"
        );
        if !six {
            continue;
        }
        sixes += 1;
        let before = total_life(&driver);
        let foe = driver.target_player(seats - 1);
        driver.cast_at("Eelectrocute", &[foe])?;
        settle(&mut driver)?;
        let card = |zone: Zone| {
            driver
                .state()
                .zone(zone)
                .iter()
                .any(|id| driver.state().object(*id).unwrap().card().name == "Eelectrocute")
        };
        assert!(card(Zone::Exile), "seed {seed}: exiled instead");
        assert!(!card(Zone::Graveyard(crate::common::pid(0))));
        assert_eq!(
            before - total_life(&driver),
            2,
            "seed {seed}: 2 damage dealt"
        );
        let foe_life = driver
            .state()
            .player(crate::common::pid(seats - 1))
            .unwrap()
            .life();
        assert_eq!(foe_life, 18, "seed {seed}: to the target");
        let setup = table(seats, seed).setup().unwrap();
        let replayed = replay(&setup, driver.engine().events()).unwrap();
        assert_eq!(replayed.state_hash(), driver.state().state_hash());
    }
    assert!(sixes > 0, "some seed rolls a 6");
    Ok(())
}
