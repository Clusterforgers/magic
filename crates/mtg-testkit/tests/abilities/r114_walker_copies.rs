//! Goal: a planeswalker type chosen as a trigger resolves (`ValueKind::R114PlaneswalkerType`, CR 205.3j) fixed into a
//! delayed trigger for the turn (CR 603.7) at 2 to 4 seats: after Leori deals combat damage, each loyalty ability of a
//! planeswalker of the chosen type is copied, one of another type is not; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::{life, replays};

const LEORI: &str = "Leori, Sparktouched Hunter";

/// A planeswalker of this type with "+1: You gain 1 life." and loyalty 3.
fn r114_walker(kind: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(kind)
            .with_type(CardType::Planeswalker)
            .with_subtype(kind)
            .with_loyalty(3)
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":1}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

fn table(seats: usize) -> Scenario {
    Scenario::new(seats).at(2, 0, Step::Main1).battlefield(
        0,
        [
            ready(card(LEORI).unwrap()),
            ready(r114_walker("Jace")),
            ready(r114_walker("Chandra")),
        ],
    )
}

/// Plays on until the stack is empty, choosing `kind` when a type is asked.
fn settle(driver: &mut Driver, kind: &str) -> Result<(), ScriptError> {
    for _ in 0..300 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| matches!(option, ChoiceOption::Label(name) if name == kind))
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn leori_copies_abilities_of_the_chosen_planeswalker_type(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(LEORI, 1)])?;
    for _ in 0..300 {
        if driver.state().turn().step() == Step::Main2 {
            break;
        }
        if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
            settle(&mut driver, "Jace")?;
        } else {
            driver.answer_passively()?;
        }
    }
    let before = life(&driver, 0);
    driver.begin_ability("Jace", 0)?;
    settle(&mut driver, "Jace")?;
    assert_eq!(
        life(&driver, 0),
        before + 2,
        "the Jace ability and its copy"
    );
    driver.begin_ability("Chandra", 0)?;
    settle(&mut driver, "Jace")?;
    assert_eq!(life(&driver, 0), before + 3, "another type is not copied");
    replays(&driver, &setup);
    Ok(())
}
