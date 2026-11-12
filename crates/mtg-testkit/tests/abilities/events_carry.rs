//! Goal: the shapes the translator now writes for carried names play out: a delayed trigger takes the object a spell exiled under its name and returns that very object at the next end step, and a `Forget` before a `Remember` empties the slot first.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::ability::Ability;
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, instant};
use mtg_testkit::trigger_cards::{artifact_with, with_rules};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn settle(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    Ok(events)
}

#[test]
fn a_delayed_trigger_returns_the_object_a_step_exiled_under_a_name() -> Result<(), ScriptError> {
    // Turn to Mist: "Exile target creature. Return that card to the battlefield at the beginning of the next end step."
    let mist = with_rules(
        &instant("Mist", 0, &[]),
        vec![rules(
            r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[
                {"Exile":{"what":{"Target":0},"as":"exiled"}},
                {"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[
                    {"Move":{"to":{"Battlefield":{}},"what":{"Bound":"exiled"}}}]}}]}}"#,
        )],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [mist])
            .battlefield(1, [ready(free_creature("Bear", 2, 2))]),
    )?;
    let target = driver.target_named("Bear")?;
    driver.cast_at("Mist", &[target])?;
    settle(&mut driver)?;
    driver.expect_zone(Zone::Exile, &["Bear"])?;
    driver.expect_zone(Zone::Battlefield, &[])?;

    driver.pass_until(Step::End)?;
    settle(&mut driver)?;
    driver.expect_zone(Zone::Battlefield, &["Bear"])?;
    driver.expect_zone(Zone::Exile, &[])?;
    let _ = pid(0);
    Ok(())
}

#[test]
fn a_forget_before_a_remember_empties_the_slot_first() -> Result<(), ScriptError> {
    // "Exile target creature; the source forgets what it remembered, then remembers it."
    let keeper = artifact_with(
        "Keeper",
        vec![rules(
            r#"{"Activated":{"cost":[],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[
                {"Exile":{"what":{"Target":0},"as":"exiled"}},
                {"Forget":{"slot":"remembered"}},
                {"Remember":{"what":{"Bound":"exiled"},"slot":"remembered"}}]}}"#,
        )],
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(keeper)])
            .battlefield(
                1,
                [
                    ready(free_creature("Bear", 2, 2)),
                    ready(free_creature("Wolf", 3, 3)),
                ],
            ),
    )?;
    let mut events = Vec::new();
    for victim in ["Bear", "Wolf"] {
        events.extend(driver.begin_activation("Keeper")?);
        let at = driver.target_named(victim)?;
        events.extend(driver.choose_targets(&[at])?);
        events.extend(settle(&mut driver)?);
    }
    driver.expect_zone(Zone::Exile, &["Bear", "Wolf"])?;
    let writes: Vec<(bool, usize)> = events
        .iter()
        .filter_map(|event| match event {
            Event::Remembered(change) if change.slot == "remembered" => {
                Some((change.replace, change.objects.len()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        writes,
        [(true, 0), (false, 1), (true, 0), (false, 1)],
        "each activation empties the slot, then adds the one exiled"
    );
    Ok(())
}
