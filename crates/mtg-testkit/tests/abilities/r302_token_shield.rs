//! Goal: a shield that doubles the tokens created under its controller's control until end of turn (Kaya, Geist Hunter's
//! −2) doubles every run of tokens made for that player this turn, at tables of two to four seats, and not a run made for
//! another player, one made with no shield, or one made in a later turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::decision::DecisionKind;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// An artifact with one activated ability that costs `{T}` and does `effects`.
fn rig(name: &str, effects: &str) -> Arc<CardDefinition> {
    let ability: Ability =
        serde_json::from_str(&format!(r#"{{"Activated":{{"cost":["Tap"],"effects":{effects}}}}}"#))
            .unwrap();
    artifact_with(name, vec![ability])
}

/// Activates `name` and passes until the stack is empty.
fn run(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck("a finished resolution".to_owned()))
}

const DOUBLER: &str = r#"[{"Shield":{"when":{"TokenCreation":{}},"by":{"Multiply":2},"duration":"UntilEndOfTurn"}}]"#;

/// The tokens each seat controls.
fn tokens(driver: &Driver, seats: usize) -> Vec<usize> {
    (0..seats)
        .map(|seat| {
            driver
                .state()
                .zone(Zone::Battlefield)
                .iter()
                .filter(|id| {
                    let object = driver.state().object(**id).unwrap();
                    object.kind() == ObjectKind::Token && object.controller() == pid(seat)
                })
                .count()
        })
        .collect()
}

/// When the tokens are made, and for whom.
#[derive(Debug, Clone, Copy)]
enum Run {
    /// Two tokens for you after the shield began.
    Yours,
    /// One token for each opponent after the shield began.
    Theirs,
    /// Two tokens for you with no shield.
    NoShield,
    /// Two tokens for you in your next turn.
    NextTurn,
}

#[rstest]
fn a_token_doubling_shield_doubles_your_runs_this_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(Run::Yours, Run::Theirs, Run::NoShield, Run::NextTurn)] made: Run,
) -> Result<(), ScriptError> {
    let who = if matches!(made, Run::Theirs) { "Opponents" } else { "You" };
    let maker = format!(
        r#"[{{"CreateToken":{{"who":"{who}","token":{{"Named":"clue"}},"count":{}}}}}]"#,
        if matches!(made, Run::Theirs) { 1 } else { 2 }
    );
    let scenario = Scenario::new(seats).battlefield(
        0,
        vec![ready(rig("Doubler", DOUBLER)), ready(rig("Maker", &maker))],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    if !matches!(made, Run::NoShield) {
        run(&mut driver, "Doubler")?;
    }
    if matches!(made, Run::NextTurn) {
        let next = u32::try_from(seats).unwrap() + 1;
        driver.idle_until(next, Step::Main1)?;
    }
    run(&mut driver, "Maker")?;
    let mut expected = vec![0; seats];
    match made {
        Run::Yours => expected[0] = 4,
        Run::Theirs => expected[1..].iter_mut().for_each(|n| *n = 1),
        Run::NoShield | Run::NextTurn => expected[0] = 2,
    }
    assert_eq!(tokens(&driver, seats), expected, "{made:?}");
    Ok(())
}
