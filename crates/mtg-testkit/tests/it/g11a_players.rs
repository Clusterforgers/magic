//! Goal: the round 11 player facts play as printed at tables of two to four seats: "each other opponent" spares the one player named, "each opponent who controls more lands than you" compares each opponent with the player whose ability asks, and a connive N draws and discards N.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// An artifact with one activated ability: tap it for `effects` with `targets`.
fn rig(name: &str, effects: &str, targets: &str) -> Arc<CardDefinition> {
    let json =
        format!(r#"{{"Activated":{{"cost":["Tap"],"effects":{effects},"targets":{targets}}}}}"#);
    let ability: Ability = serde_json::from_str(&json).unwrap();
    artifact_with(name, vec![ability])
}

/// Activates `name`, aims at `aim` when asked, takes the first cards when asked, and passes until the stack is empty.
fn run(driver: &mut Driver, name: &str, aim: Option<Target>) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        let done = matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty();
        if done {
            return Ok(());
        }
        let answer = match (&decision.kind, aim) {
            (DecisionKind::ChooseTargets(_), Some(aim)) => {
                Answer::ChooseTargets(mtg_engine::choose::TargetChoice::Targets(vec![aim]))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::NoSuchCard {
        name: "a finished resolution".to_owned(),
        place: "60 answers",
    })
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

const SPREAD: &str = r#"[{"DealDamage":{"amount":3,"to":{"PlayersWhere":{"who":"Opponents","test":{"Not":{"Among":{"Target":0}}}}}}}]"#;

#[rstest]
fn each_other_opponent_spares_the_one_named(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let named = seats - 1;
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(
        0,
        vec![ready(rig("Spread", SPREAD, r#"[{"what":"Opponent"}]"#))],
    ))?;
    run(&mut driver, "Spread", Some(Target::Player(pid(named))))?;
    assert_eq!(life(&driver, 0), 20);
    assert_eq!(life(&driver, named), 20);
    for seat in 1..named {
        assert_eq!(life(&driver, seat), 17, "seat {seat} of {seats}");
    }
    Ok(())
}

const MORE_LANDS: &str = r#"[{"GainLife":{"amount":{"Players":{"PlayersWhere":{"who":"Opponents","test":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},"op":"Gt","right":{"Count":[{"Type":"Land"},{"ControlledBy":"Asker"}]}}}}}}}}]"#;

/// Seat 0 controls the rig and one Forest; seat `seat` controls `seat` Forests.
#[rstest]
fn the_opponents_with_more_lands_are_compared_with_the_asker(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        vec![ready(rig("Survey", MORE_LANDS, "[]")), ready(forest())],
    );
    for seat in 1..seats {
        scenario =
            scenario.battlefield(seat, (0..seat).map(|_| ready(forest())).collect::<Vec<_>>());
    }
    let mut driver = Driver::from_scenario(scenario)?;
    run(&mut driver, "Survey", None)?;
    // Seat 1 has as many lands as seat 0; each later seat has more.
    let more = i32::try_from(seats - 2).unwrap();
    assert_eq!(life(&driver, 0), 20 + more);
    Ok(())
}

#[rstest]
fn connive_two_draws_two_and_discards_two(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let effects = r#"[{"Connive":{"what":{"Target":0},"count":2}}]"#;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                vec![
                    ready(rig(
                        "Scheme",
                        effects,
                        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
                    )),
                    ready(free_creature("Spy", 1, 1)),
                ],
            )
            .library(
                0,
                vec![
                    free_creature("Ogre", 3, 3),
                    free_creature("Imp", 1, 1),
                    forest(),
                ],
            ),
    )?;
    let aim = driver.target_named("Spy")?;
    run(&mut driver, "Scheme", Some(aim))?;
    let graveyard = driver.state().zone(Zone::Graveyard(pid(0))).len();
    assert_eq!(graveyard, 2);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 0);
    let spy = driver.id_of("Spy").unwrap();
    let spy = driver.state().object(spy).unwrap().permanent().unwrap();
    assert_eq!(
        spy.counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne),
        2
    );
    Ok(())
}
