//! Goal: a rule that keeps planeswalkers with 0 loyalty (`GameRule::ZeroLoyaltyStays`, CR 704.5i) keeps only those its
//! selector picks for its controller (Sanctum Lurker: planeswalkers you control), an opponent's still go to the
//! graveyard, and once the rule's source leaves a kept planeswalker with 0 loyalty is put into its owner's graveyard by
//! the next state-based check, at every table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// A planeswalker with loyalty 3 and "[-3]: You gain 1 life."
fn walker(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Planeswalker)
            .with_loyalty(3)
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Loyalty":{"cost":{"Minus":3},"effects":[{"GainLife":{"amount":1}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

/// An instant: "Destroy target creature." for nothing.
fn murder() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Murder")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn loyalty(driver: &Driver, name: &str) -> Option<u32> {
    let id = driver.id_of(name).ok()?;
    let object = driver.state().object(id)?;
    (object.zone() == Zone::Battlefield)
        .then(|| object.permanent().map(|p| p.counters_of(&CounterKind::Loyalty)))
        .flatten()
}

#[rstest]
fn a_walker_of_the_lurkers_controller_stays_with_zero_loyalty(
    #[values(true, false)] lurker: bool,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(walker("Mine"))];
    if lurker {
        mine.push(ready(card("Sanctum Lurker").unwrap()));
    }
    let mut scenario = Scenario::new(seats).battlefield(0, mine).hand(0, [murder()]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(walker(&format!("Theirs {seat}")))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Mine", 0)?;
    settle(&mut driver)?;
    assert_eq!(loyalty(&driver, "Mine"), lurker.then_some(0));
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Mine".to_owned()),
        !lurker
    );
    if lurker {
        // The rule ends with its source: the walker goes at the next check.
        let target = driver.target_named("Sanctum Lurker")?;
        driver.cast_at("Murder", &[target])?;
        settle(&mut driver)?;
        assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Mine".to_owned()));
    }
    Ok(())
}

#[rstest]
fn an_opponents_walker_is_not_kept(#[values(2, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 1, mtg_engine::turn::Step::Main1)
        .battlefield(0, [ready(card("Sanctum Lurker").unwrap())])
        .battlefield(1, [ready(walker("Theirs"))]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(walker(&format!("Other {seat}")))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Theirs", 0)?;
    settle(&mut driver)?;
    assert!(names(driver.state(), Zone::Graveyard(pid(1))).contains(&"Theirs".to_owned()));
    Ok(())
}
