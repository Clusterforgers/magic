//! Goal: a copy "except it has this ability" made by a reflexive trigger keeps the printed ability that set the trigger up, not the reflexive trigger itself, so the copy can copy again (Aurora Shifter, CR 707.9b, 603.12); at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::choose::{CardsChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::sized_creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A 1/1 with "{0}: when you do, this becomes a copy of another target creature, except `except`": the copy is made by
/// a reflexive trigger, or by the activated ability itself when `reflexive` is false.
fn shifter(reflexive: bool) -> Arc<CardDefinition> {
    let copy = r#"{"BecomeCopy":{"what":"This","of":{"Target":0},"except":["ThisAbility"],"duration":"Forever"}}"#;
    let target = r#"[{"what":{"Object":[{"Type":"Creature"},"Other"]}}]"#;
    let json = if reflexive {
        format!(
            r#"{{"Activated":{{"cost":[],"effects":[{{"Reflexive":{{"targets":{target},"effects":[{copy}]}}}}]}}}}"#
        )
    } else {
        format!(r#"{{"Activated":{{"cost":[],"targets":{target},"effects":[{copy}]}}}}"#)
    };
    sized_creature_with(
        "Shifter",
        1,
        1,
        vec![serde_json::from_str::<Ability>(&json).unwrap()],
    )
}

/// Activates `name` and answers until the stack is empty, aiming at `aim` when it is legal.
fn activate(driver: &mut Driver, name: &str, aim: Target) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseCards(o) => Answer::ChooseCards(CardsChoice(
                o.from.iter().copied().take(o.max as usize).collect(),
            )),
            DecisionKind::ChooseTargets(o) if o.legal.contains(&aim) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![aim]))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the copy".into()))
}

#[rstest]
fn a_reflexive_copy_keeps_the_ability_that_made_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] reflexive: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(
        0,
        vec![
            ready(shifter(reflexive)),
            ready(card("Grizzly Bears").unwrap()),
            ready(card("Llanowar Elves").unwrap()),
        ],
    ))?;
    let shifter = driver.id_of("Shifter")?;
    let bears = driver.id_of("Grizzly Bears")?;
    let aim = Target::Object(driver.state().object(bears).unwrap().reference());
    activate(&mut driver, "Shifter", aim)?;
    assert_eq!(
        driver.state().object(shifter).unwrap().card().name,
        "Grizzly Bears"
    );
    // The kept ability is the activated one: the copy activates it again and becomes the Elves.
    let elves = driver.id_of("Llanowar Elves")?;
    let aim = Target::Object(driver.state().object(elves).unwrap().reference());
    activate(&mut driver, "Grizzly Bears", aim)?;
    assert_eq!(
        driver.state().object(shifter).unwrap().card().name,
        "Llanowar Elves"
    );
    Ok(())
}
