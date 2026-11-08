//! Goal: "do this only once each turn" on a trigger whose first step is a choice is spent only when the choice is done: a declined "may" leaves it for a later trigger that turn, a done one ends it for the turn, and a limit spent as it triggers ends it either way, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::Answer;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::swamp;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::names;

fn card(name: &str, kind: CardType, json: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(json).unwrap();
    Arc::new(CardDefinition::new(name).with_type(kind).with_rules(rules))
}

/// "{0}: You gain 1 life."
fn spring() -> Arc<CardDefinition> {
    card(
        "Spring",
        CardType::Artifact,
        r#"{"Activated":{"cost":[],"effects":[{"GainLife":{"amount":1}}]}}"#,
    )
}

/// "Whenever you gain life, you may draw a card. Do this only once each turn." with the limit given.
fn watcher(limit: &str) -> Arc<CardDefinition> {
    card(
        "Watcher",
        CardType::Artifact,
        &format!(
            r#"{{"Triggered":{{"when":{{"LifeGained":{{"who":"You"}}}},"limit":"{limit}","effects":[{{"May":{{"effects":[{{"Draw":{{}}}}]}}}}]}}}}"#
        ),
    )
}

/// Gains life three times, answering the questions in turn (no once the answers run out); the questions asked and the
/// cards drawn.
fn play(seats: usize, limit: &str, answers: &[bool]) -> Result<(usize, usize), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .library(0, (0..5).map(|_| swamp()))
            .battlefield(0, [ready(spring()), ready(watcher(limit))]),
    )?;
    let mut asked = 0;
    for _ in 0..3 {
        driver.begin_activation("Spring")?;
        for _ in 0..200 {
            if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
                break;
            }
            if driver.decision()?.kind.name() == "yes-no" {
                let yes = answers.get(asked).copied().unwrap_or(false);
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            } else {
                driver.answer_passively()?;
            }
        }
    }
    driver.expect_life(0, 23)?;
    Ok((asked, names(driver.state(), Zone::Hand(PlayerId(0))).len()))
}

#[rstest]
#[case::declined_then_done("OncePerTurnDone", &[false, true], 2, 1)]
#[case::done_at_once("OncePerTurnDone", &[true], 1, 1)]
#[case::never_done("OncePerTurnDone", &[], 3, 0)]
#[case::spent_as_it_triggers("OncePerTurn", &[false, true], 1, 0)]
fn a_limit_on_a_choice_is_spent_when_the_choice_is_done(
    #[case] limit: &str,
    #[case] answers: &[bool],
    #[case] asked: usize,
    #[case] drawn: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    assert_eq!(play(seats, limit, answers)?, (asked, drawn));
    Ok(())
}
