//! Goal: "As this artifact enters, look at an opponent's hand, then choose any card name." asks the name with the
//! cards of the hand looked at (Sorcerous Spyglass, Anointed Peacekeeper) at tables of two to four seats: the opponent
//! is chosen first when there are several, the name asked lists that hand, the chosen name is remembered, and the
//! log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::choose::{NameChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::Chosen;
use mtg_engine::replay;
use mtg_testkit::cards::{creature, instant, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::casting::lands;

/// Answers until the name is asked; an opponent asked for is the seat after the caster.
fn r6003_until_named(driver: &mut Driver) -> Result<Vec<String>, ScriptError> {
    for _ in 0..64 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::ChooseName(options) => return Ok(options.looked.clone()),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseOption(_) => Answer::ChooseOption(OptionChoice(0)),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("no name was asked".into()))
}

#[rstest]
#[case::spyglass_two("Sorcerous Spyglass", 2)]
#[case::spyglass_three("Sorcerous Spyglass", 3)]
#[case::spyglass_four("Sorcerous Spyglass", 4)]
#[case::peacekeeper_two("Anointed Peacekeeper", 2)]
#[case::peacekeeper_four("Anointed Peacekeeper", 4)]
fn r6003_the_name_is_asked_with_the_hand_looked_at(
    #[case] name: &str,
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, lands(3, plains))
        .hand(0, vec![card(name).unwrap()]);
    for seat in 1..seats {
        scenario = scenario.hand(
            seat,
            vec![
                creature("Elf", 0, &[Color::Green], 1, 1),
                instant("Shock", 0, &[Color::Red]),
            ],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(name)?;
    let looked = r6003_until_named(&mut driver)?;
    assert_eq!(
        looked,
        ["Elf", "Shock"],
        "the hand of the opponent looked at"
    );
    driver.answer(Answer::ChooseName(NameChoice("Shock".to_owned())))?;
    let id = driver.id_of(name)?;
    let chosen = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .chosen()
        .cloned();
    assert_eq!(chosen, Some(Chosen::Name("Shock".to_owned())));
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
