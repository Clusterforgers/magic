//! Goal: "Whenever this creature enlists a creature" (`Did { Enlisted }`) triggers once when the creature's enlist taps another creature, never when its controller declines, and another enlisting creature does not set it off; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use crate::macro_keywords::{life, macro_keyword, play, size};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, keyworded};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An enlisting creature with "Whenever ~ enlists a creature, you gain 100 life."
fn guardian(name: &str) -> Arc<CardDefinition> {
    let card = Arc::unwrap_or_clone(keyworded(name, 2, 9, &[macro_keyword("enlist", 0)]));
    let rule = r#"{"Triggered":{"when":{"Did":{"action":"Enlisted","what":["This"]}},"effects":[{"GainLife":{"amount":100}}]}}"#;
    Arc::new(card.with_rules(serde_json::from_str::<Ability>(rule).unwrap()))
}

#[rstest]
#[case::guardian_enlists("Guardian", true, 100, 3)]
#[case::guardian_declines("Guardian", false, 0, 0)]
#[case::another_enlists("Other", true, 0, 3)]
fn an_enlist_is_an_event_of_the_enlisting_creature(
    #[case] attacker: &str,
    #[case] enlist: bool,
    #[case] gained: i32,
    #[case] added: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        vec![
            ready(guardian("Guardian")),
            ready(keyworded("Other", 2, 9, &[macro_keyword("enlist", 0)])),
            ready(free_creature("Helper", 3, 3)),
        ],
    );
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            vec![ready(free_creature(&format!("Wall {seat}"), 0, 9))],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before = life(&driver, 0);
    let hook = move |driver: &Driver, decision: &Decision| match &decision.kind {
        DecisionKind::ChooseCards(options) if decision.player == pid(0) => {
            let helper = options
                .from
                .iter()
                .find(|id| driver.state().object(**id).unwrap().card().name == "Helper")?;
            Some(Answer::ChooseCards(CardsChoice(vec![*helper])))
        }
        DecisionKind::YesNo(_) if decision.player == pid(0) => {
            Some(Answer::YesNo(YesNoChoice(enlist)))
        }
        _ => None,
    };
    play(&mut driver, &[attacker], &[], &hook, Step::EndCombat)?;
    assert_eq!(life(&driver, 0) - before, gained);
    assert_eq!(size(&driver, attacker).0, 2 + added);
    Ok(())
}
