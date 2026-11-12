//! Goal: a shield over a kind of recipient ("prevent all damage that would be dealt to creatures you control this
//! turn") judges the kind as each damage event happens, at tables of two to four seats: a creature cast or stolen
//! after the shield began is protected, one already there is protected, and another player's creature is not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// An artifact with one activated ability that costs `{T}`: `effects` and `targets` as the card DSL writes them.
fn rig(name: &str, effects: &str, targets: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":{effects},"targets":{targets}}}}}"#
    ))
    .unwrap();
    artifact_with(name, vec![ability])
}

/// Activates `name`, aims at `aim` when asked, and passes until the stack is empty.
fn run(driver: &mut Driver, name: &str, aim: Option<Target>) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        let answer = match (&decision.kind, aim) {
            (DecisionKind::ChooseTargets(_), Some(aim)) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![aim]))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("a finished resolution".to_owned()))
}

const CREATURE: &str = r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#;
const WARD: &str = r#"[{"Shield":{"by":"Prevent","duration":"UntilEndOfTurn","when":{"Damage":{"to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}}}]"#;
const BOLT: &str = r#"[{"DealDamage":{"amount":5,"to":{"Target":0}}}]"#;
const STEAL: &str = r#"[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}}]"#;

/// How the creature that is bolted came to be where it is.
#[derive(Debug, Clone, Copy)]
enum Arrival {
    /// Yours before the shield began.
    Before,
    /// Cast by you after the shield began.
    Cast,
    /// Another player's, stolen after the shield began.
    Stolen,
    /// Another player's, still theirs.
    Theirs,
}

#[rstest]
fn a_shield_over_creatures_you_control_covers_those_that_arrive_later(
    #[values(2, 3, 4)] seats: usize,
    #[values(Arrival::Before, Arrival::Cast, Arrival::Stolen, Arrival::Theirs)] arrival: Arrival,
) -> Result<(), ScriptError> {
    let other = seats - 1;
    let mut mine = vec![
        ready(rig("Ward", WARD, "[]")),
        ready(rig("Bolt", BOLT, CREATURE)),
        ready(rig("Steal", STEAL, CREATURE)),
    ];
    if matches!(arrival, Arrival::Before) {
        mine.push(ready(free_creature("Victim", 2, 2)));
    }
    let mut scenario = Scenario::new(seats).battlefield(0, mine);
    if matches!(arrival, Arrival::Stolen | Arrival::Theirs) {
        scenario = scenario.battlefield(other, vec![ready(free_creature("Victim", 2, 2))]);
    }
    if matches!(arrival, Arrival::Cast) {
        scenario = scenario.hand(0, vec![free_creature("Victim", 2, 2)]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    run(&mut driver, "Ward", None)?;
    match arrival {
        Arrival::Cast => {
            driver.cast("Victim")?;
            for _ in 0..10 {
                if driver.state().stack().is_empty() {
                    break;
                }
                driver.pass()?;
            }
        }
        Arrival::Stolen => {
            let aim = driver.target_named("Victim")?;
            run(&mut driver, "Steal", Some(aim))?;
        }
        Arrival::Before | Arrival::Theirs => {}
    }
    let aim = driver.target_named("Victim")?;
    run(&mut driver, "Bolt", Some(aim))?;
    let owner = if matches!(arrival, Arrival::Stolen | Arrival::Theirs) {
        other
    } else {
        0
    };
    let dead = driver
        .state()
        .zone(Zone::Graveyard(pid(owner)))
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == "Victim");
    assert_eq!(dead, matches!(arrival, Arrival::Theirs), "{arrival:?}");
    Ok(())
}
