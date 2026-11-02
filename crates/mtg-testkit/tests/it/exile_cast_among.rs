//! Goal: Etali, Primal Storm exiles the top card of every library and its controller may cast any number of the exiled spells for free, at tables of two to four seats: each nonland card is asked about in turn, a land is never asked and stays in exile, the cards declined stay in exile, every spell cast goes on the stack, and a spell with X is cast with X 0.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::Event;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::names;

/// The top card of a library from a spec: `land`, `fireball` (X spell with a target) or a bear named by the spec.
fn top(spec: &str) -> Arc<CardDefinition> {
    match spec {
        "land" => forest(),
        "fireball" => card("Fireball").unwrap(),
        name => creature(name, 3, &[], 2, 2),
    }
}

/// Etali attacks seat 1 and the trigger is answered with `cast` (the names to cast). Returns the game, the names asked
/// about and the events of the answers.
fn attack(tops: &[&str], cast: &[&str]) -> Result<(Driver, Vec<String>, Vec<Event>), ScriptError> {
    let mut scenario = Scenario::new(tops.len())
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Etali, Primal Storm").unwrap())]);
    for (seat, spec) in tops.iter().enumerate() {
        scenario = scenario.library(seat, vec![top(spec)]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Etali, Primal Storm", 1)])?;
    let (mut asked, mut events) = (Vec::new(), Vec::new());
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::YesNo(question) => {
                let name = question
                    .text
                    .strip_prefix("Cast ")
                    .and_then(|rest| rest.strip_suffix(" without paying its mana cost?"))
                    .expect("a free cast is asked")
                    .to_owned();
                let yes = cast.contains(&name.as_str());
                asked.push(name);
                events.extend(driver.answer(Answer::YesNo(YesNoChoice(yes)))?);
            }
            DecisionKind::ChooseTargets(_) => {
                let foe = driver.target_player(1);
                events.extend(driver.choose_targets(&[foe])?);
            }
            DecisionKind::Priority(_) if asked.is_empty() && !driver.state().stack().is_empty() => {
                driver.pass()?;
            }
            _ => break,
        }
    }
    Ok((driver, asked, events))
}

#[rstest]
#[case::two_seats_none(&["Alpha", "Beta"], &[], &["Alpha", "Beta"], 0)]
#[case::two_seats_one(&["Alpha", "Beta"], &["Beta"], &["Alpha", "Beta"], 1)]
#[case::two_seats_all(&["Alpha", "Beta"], &["Alpha", "Beta"], &["Alpha", "Beta"], 2)]
#[case::land_is_not_asked(&["Alpha", "land", "Gamma"], &["Alpha", "Gamma"], &["Alpha", "Gamma"], 2)]
#[case::only_lands(&["land", "land"], &[], &[], 0)]
#[case::four_seats_some(&["Alpha", "land", "Gamma", "Delta"], &["Alpha", "Delta"], &["Alpha", "Gamma", "Delta"], 2)]
#[case::an_x_spell_with_x_0(&["fireball", "Beta", "land"], &["Fireball", "Beta"], &["Fireball", "Beta"], 2)]
fn any_number_of_the_exiled_spells_are_cast_free_and_lands_are_not_asked(
    #[case] tops: &[&str],
    #[case] cast: &[&str],
    #[case] asked: &[&str],
    #[case] casts: usize,
) -> Result<(), ScriptError> {
    let (driver, got, events) = attack(tops, cast)?;
    assert_eq!(got, asked);
    let spells: Vec<&Event> = events
        .iter()
        .filter(|event| matches!(event, Event::SpellCast { .. }))
        .collect();
    assert_eq!(spells.len(), casts);
    for spell in spells {
        if let Event::SpellCast { choices, .. } = spell {
            assert_eq!(choices.x.unwrap_or(0), 0);
        }
    }
    let stack = names(driver.state(), Zone::Stack);
    assert_eq!(stack.len(), casts, "{stack:?}");
    let exile = names(driver.state(), Zone::Exile);
    assert_eq!(exile.len(), tops.len() - casts, "{exile:?}");
    assert_eq!(
        exile.iter().filter(|name| *name == "Forest").count(),
        tops.iter().filter(|spec| **spec == "land").count()
    );
    Ok(())
}
