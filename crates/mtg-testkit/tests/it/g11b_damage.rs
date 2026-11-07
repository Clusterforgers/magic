//! Goal: the damage pack g11b plays as printed at tables of two to four seats: damage that can't be prevented passes a
//! prevention effect on the creature and kills it, the same damage without the flag is prevented, and an unshielded
//! creature dies either way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// An artifact with one activated ability that costs `{T}`: `effects` and `targets` as the card DSL writes them.
fn rig(name: &str, effects: &str, targets: &str) -> Arc<CardDefinition> {
    artifact_with(
        name,
        vec![ability(&format!(
            r#"{{"Activated":{{"cost":["Tap"],"effects":{effects},"targets":{targets}}}}}"#
        ))],
    )
}

/// Activates `name`, aims at `aim`, and passes until the stack is empty.
fn run(driver: &mut Driver, name: &str, aim: Target) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
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
                Answer::ChooseTargets(TargetChoice::Targets(vec![aim]))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("a finished resolution".to_owned()))
}

fn names_in(driver: &Driver, zone: Zone) -> Vec<String> {
    driver
        .state()
        .zone(zone)
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect()
}

const CREATURE: &str = r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#;

/// A 4/4 creature; `shield` is a replacement that prevents damage dealt to it, when given.
fn victim(shield: Option<&str>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Victim")
        .with_type(CardType::Creature)
        .with_power_toughness(4, 4);
    if let Some(shield) = shield {
        card = card.with_rules(ability(shield));
    }
    Arc::new(card)
}

const PREVENT_ALL: &str = r#"{"Replacement":{"by":"Prevent","when":{"Damage":{"to":"This"}}}}"#;
const PREVENT_HALF: &str =
    r#"{"Replacement":{"by":"PreventHalf","when":{"Damage":{"to":"This"}}}}"#;

#[rstest]
fn damage_that_cant_be_prevented_passes_the_prevention(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some(PREVENT_ALL), Some(PREVENT_HALF))] shield: Option<&str>,
    #[values(true, false)] unpreventable: bool,
) -> Result<(), ScriptError> {
    let owner = seats - 1;
    let flag = if unpreventable {
        r#","unpreventable":true"#
    } else {
        ""
    };
    let effects = format!(r#"[{{"DealDamage":{{"amount":5,"to":{{"Target":0}}{flag}}}}}]"#);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, vec![ready(rig("Bolt", &effects, CREATURE))])
            .battlefield(owner, vec![ready(victim(shield))]),
    )?;
    let aim = driver.target_named("Victim")?;
    run(&mut driver, "Bolt", aim)?;
    let dead = names_in(&driver, Zone::Graveyard(pid(owner))).contains(&"Victim".to_owned());
    assert_eq!(dead, unpreventable || shield.is_none(), "{shield:?}");
    Ok(())
}
