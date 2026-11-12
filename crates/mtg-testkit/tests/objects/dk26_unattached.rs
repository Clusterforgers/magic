//! Goal: "Whenever this Equipment becomes unattached from a permanent, sacrifice that permanent" triggers when the
//! Equipment moves to another creature and when it leaves the battlefield, and its controller sacrifices the permanent it
//! left only while they control it; an Equipment that was attached to nothing, or a host that left first, sacrifices
//! nothing else.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

use crate::common::{names, pid};

const TRIGGER: &str = r#"{"Triggered":{"when":{"Unattached":{"what":["This"]}},"effects":[{"Sacrifice":{"filter":[{"Is":"Triggering"}]}}]}}"#;
const EQUIP: &str = r#"{"Keyword":{"Equip":"{0}"}}"#;
const BREAK: &str = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn exoskeleton() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Exoskeleton")
            .with_type(CardType::Artifact)
            .with_subtype("Equipment")
            .with_rules(rules(TRIGGER))
            .with_rules(rules(EQUIP)),
    )
}

fn breaker() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Breaker")
            .with_type(CardType::Artifact)
            .with_rules(rules(BREAK)),
    )
}

/// Answers every question to the end, choosing `target` wherever a target is asked.
fn play_out(driver: &mut Driver, target: &str) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        let done = matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty();
        if done {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                let Target::Object(chosen) = driver.target_named(target)? else {
                    panic!("an object");
                };
                Answer::ChooseTargets(TargetChoice::Targets(vec![Target::Object(chosen)]))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("unattached".into()))
}

/// Seat 0: the Exoskeleton (attached to the creature at `host` of `seat`'s battlefield, or to nothing), Ant, Bee and the
/// Breaker; seat 1: Wasp.
fn table(host: Option<(usize, usize)>) -> Scenario {
    let equipment = match host {
        Some((seat, place)) => attached_to(ready(exoskeleton()), seat, place),
        None => ready(exoskeleton()),
    };
    Scenario::new(2)
        .battlefield(
            0,
            [
                ready(free_creature("Ant", 1, 1)),
                ready(free_creature("Bee", 1, 1)),
                ready(breaker()),
                equipment,
            ],
        )
        .battlefield(1, [ready(free_creature("Wasp", 1, 1))])
}

fn graveyards(driver: &Driver) -> Vec<String> {
    let mut all = names(driver.state(), Zone::Graveyard(pid(0)));
    all.extend(names(driver.state(), Zone::Graveyard(pid(1))));
    all.sort();
    all
}

#[rstest]
#[case::moved_from_my_creature(Some((0, 0)), &["Ant"])]
#[case::moved_from_their_creature(Some((1, 0)), &[])]
#[case::attached_to_nothing(None, &[])]
fn equipping_another_creature_sacrifices_the_one_it_left(
    #[case] host: Option<(usize, usize)>,
    #[case] dead: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(host))?;
    driver.begin_activation("Exoskeleton")?;
    play_out(&mut driver, "Bee")?;
    assert_eq!(graveyards(&driver), dead);
    Ok(())
}

#[rstest]
#[case::my_creature(Some((0, 0)), &["Ant", "Exoskeleton"])]
#[case::their_creature(Some((1, 0)), &["Exoskeleton"])]
#[case::attached_to_nothing(None, &["Exoskeleton"])]
fn the_equipment_leaving_sacrifices_the_permanent_it_left(
    #[case] host: Option<(usize, usize)>,
    #[case] dead: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(host))?;
    driver.begin_activation("Breaker")?;
    play_out(&mut driver, "Exoskeleton")?;
    assert_eq!(graveyards(&driver), dead);
    Ok(())
}

#[test]
fn a_host_that_dies_first_takes_nothing_else_with_it() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(Some((0, 0))))?;
    driver.begin_activation("Breaker")?;
    play_out(&mut driver, "Ant")?;
    assert_eq!(graveyards(&driver), ["Ant"]);
    let battlefield = names(driver.state(), Zone::Battlefield);
    assert!(battlefield.contains(&"Bee".to_owned()));
    assert!(battlefield.contains(&"Exoskeleton".to_owned()));
    Ok(())
}
