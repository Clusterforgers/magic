//! Goal: under a life floor "whenever you lose life" triggers see the life actually lost, not the damage dealt, and none fires when the floor leaves no loss, at two and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn rule(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A creature that keeps its controller's life total at 1 or more against damage.
fn guard() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Guard")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rule(
                r#"{"Rule":{"rule":{"LifeFloor":{"who":"You","floor":1}}}}"#,
            )),
    )
}

/// A creature that drains the first opponent for the life its controller loses.
fn drain() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Drain")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rule(
                r#"{"Triggered":{"when":{"LifeLost":{"who":"You"}},"targets":[{"what":"Opponent"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":"Triggering"}}]}}"#,
            )),
    )
}

/// A free instant that deals `damage` to its target.
fn zap(damage: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Zap")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_core::mana::ManaCost::parse("{R}").unwrap())
            .with_rules(rule(&format!(
                r#"{{"Spell":{{"targets":[{{"what":"AnyTarget"}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{damage}}}}}]}}}}"#
            ))),
    )
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        match &driver.decision()?.kind {
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
    Ok(())
}

#[rstest]
#[case::the_floor_cuts_the_loss(5, 8, 4)]
#[case::the_whole_damage_is_lost_above_the_floor(20, 8, 8)]
#[case::no_loss_at_the_floor(1, 8, 0)]
fn the_trigger_reads_the_life_lost_under_a_floor(
    #[values(2, 4)] seats: usize,
    #[case] life_before: i32,
    #[case] damage: u32,
    #[case] drained: i32,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .life(0, life_before)
        .hand(0, vec![zap(damage)])
        .battlefield(0, vec![ready(guard()), ready(drain()), ready(mountain())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 30]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let me = driver.target_player(0);
    driver.cast_at("Zap", &[me])?;
    settle(&mut driver)?;
    assert_eq!(
        life(&driver, 0),
        life_before.min(life_before - drained).max(1)
    );
    assert_eq!(life(&driver, 1), 20 - drained);
    Ok(())
}
