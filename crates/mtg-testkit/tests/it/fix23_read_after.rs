//! Goal: what an effect reads after an earlier step of its own resolution plays as printed at two to four seats: the counters of a permanent the ability exiled first are its last known counters (Sphere of Annihilation), and the result of a die is the number rolled, with only the rows it falls in taken (Adorable Kitten, Dissatisfied Customer).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::Event;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// An artifact whose one ability taps it for the effects, as the card DSL writes them.
fn rig(effects: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":{effects}}}}}"#
    ))
    .unwrap();
    artifact_with("Rig", vec![ability])
}

/// Activates the rig and passes until its ability has resolved.
fn activate(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.begin_activation("Rig")?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::NoSuchCard {
        name: "a finished resolution".to_owned(),
        place: "40 answers",
    })
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

/// Sphere of Annihilation's order: the source is exiled first, then the count of its counters is read.
const EXILED_FIRST: &str = r#"[{"Exile":{"what":"This"}},{"GainLife":{"amount":{"Counters":{"counter":{"Other":"void"},"on":"This"}}}}]"#;

#[rstest]
fn the_counters_of_the_source_exiled_by_its_own_ability_are_its_last_known(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2, 5)] counters: u32,
) -> Result<(), ScriptError> {
    let rig = with_counters(
        ready(rig(EXILED_FIRST)),
        CounterKind::Other("void".into()),
        counters,
    );
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, vec![rig]))?;
    activate(&mut driver)?;
    assert_eq!(life(&driver), 20 + i32::try_from(counters).unwrap());
    Ok(())
}

/// Adorable Kitten: "roll a six-sided die. You gain life equal to the result."
const KITTEN: &str = r#"[{"RollDice":{"sides":6,"results":[{"from":1,"effects":[{"GainLife":{"amount":{"DieResult":"Only"}}}]}]}}]"#;
/// Dissatisfied Customer: "roll a six-sided die. If the result is 3 or less, you lose that much life."
const CUSTOMER: &str = r#"[{"RollDice":{"sides":6,"results":[{"from":1,"to":3,"effects":[{"LoseLife":{"amount":{"DieResult":"Only"}}}]}]}}]"#;

#[rstest]
#[case::gain_the_result(KITTEN, |rolled: i32| rolled)]
#[case::lose_it_at_three_or_less(CUSTOMER, |rolled: i32| if rolled <= 3 { -rolled } else { 0 })]
fn the_result_of_the_die_is_the_number_rolled(
    #[case] effects: &str,
    #[case] change: fn(i32) -> i32,
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 4, 5, 6, 7, 8)] seed: u64,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .seed(seed)
        .battlefield(0, vec![ready(rig(effects))]);
    let mut driver = Driver::from_scenario(scenario)?;
    activate(&mut driver)?;
    let rolled: Vec<u32> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled { result, .. } => Some(*result),
            _ => None,
        })
        .collect();
    let [rolled] = rolled[..] else {
        panic!("one die: {rolled:?}");
    };
    let rolled = i32::try_from(rolled).unwrap();
    assert!((1..=6).contains(&rolled));
    assert_eq!(life(&driver), 20 + change(rolled));
    Ok(())
}
