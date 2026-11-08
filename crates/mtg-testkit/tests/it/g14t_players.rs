//! Goal: the player facts round 14 reads play as printed at tables of two to four seats: "each opponent who has more cards in hand than you" compares each opponent with the asker, "each opponent who controls a creature with power 4 or more" counts the opponents with one, and the cards in all the opponents' hands are counted together.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::decision::DecisionKind;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// An artifact with one activated ability: tap it for `effects`.
fn rig(name: &str, effects: &str) -> Arc<CardDefinition> {
    let json = format!(r#"{{"Activated":{{"cost":["Tap"],"effects":{effects},"targets":[]}}}}"#);
    let ability: Ability = serde_json::from_str(&json).unwrap();
    artifact_with(name, vec![ability])
}

/// Activates `name` and passes until the stack is empty.
fn run(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::NoSuchCard {
        name: "a finished resolution".to_owned(),
        place: "60 answers",
    })
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn gain_per(test: &str) -> String {
    format!(
        r#"[{{"GainLife":{{"amount":{{"Players":{{"PlayersWhere":{{"who":"Opponents","test":{test}}}}}}}}}}}]"#
    )
}

/// Seat 0 holds one card; seat `n` holds `n` cards, so the seats above seat 1 hold more than seat 0.
#[rstest]
fn the_opponents_with_more_cards_in_hand_are_compared_with_the_asker(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let test = r#"{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Gt","right":{"Count":[{"In":"Hand"},{"OwnedBy":"Asker"}]}}}"#;
    let mut scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(rig("Survey", &gain_per(test)))])
        .hand(0, vec![forest()]);
    for seat in 1..seats {
        scenario = scenario.hand(seat, (0..seat).map(|_| forest()).collect::<Vec<_>>());
    }
    let mut driver = Driver::from_scenario(scenario)?;
    run(&mut driver, "Survey")?;
    let more = i32::try_from(seats - 2).unwrap();
    assert_eq!(life(&driver, 0), 20 + more);
    Ok(())
}

#[rstest]
fn the_opponents_with_a_big_creature_are_counted(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let test = r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"Compare":{"op":"Ge","stat":"Power","to":4}},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#;
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        vec![
            ready(rig("Survey", &gain_per(test))),
            ready(free_creature("Ogre", 5, 5)),
        ],
    );
    // Only seat 1 has a creature with power 4 or more; the others hold small ones.
    scenario = scenario.battlefield(1, vec![ready(free_creature("Giant", 4, 4))]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, vec![ready(free_creature("Mouse", 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    run(&mut driver, "Survey")?;
    assert_eq!(life(&driver, 0), 21);
    Ok(())
}

#[rstest]
fn the_cards_in_all_the_opponents_hands_are_counted_together(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let effects = r#"[{"GainLife":{"amount":{"Count":[{"In":"Hand"},{"OwnedBy":"Opponents"}]}}}]"#;
    let mut scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(rig("Survey", effects))])
        .hand(0, vec![forest()]);
    for seat in 1..seats {
        scenario = scenario.hand(seat, vec![forest(), forest()]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    run(&mut driver, "Survey")?;
    let others = i32::try_from(seats - 1).unwrap();
    assert_eq!(life(&driver, 0), 20 + 2 * others);
    Ok(())
}
