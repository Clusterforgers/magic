//! Goal: helpers for the tests of alternative costs and cast permissions (this is not a test module itself).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::{Driver, ScriptError, ready};

/// `count` untapped copies of a land.
pub fn lands(count: usize, land: fn() -> Arc<CardDefinition>) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

/// Asserts that the announcement begun after `before` (the log length and state hash then) was taken back: the log
/// gained only its `AnnouncementRewound` for `reason` and the state is as it was.
pub fn assert_rewound(
    driver: &Driver,
    before: (usize, u64),
    reason: mtg_engine::decision::Unplayable,
) {
    let (mark, hash) = before;
    let logged = &driver.engine().events()[mark..];
    assert!(
        matches!(logged, [Event::AnnouncementRewound { reason: why, .. }] if *why == reason),
        "only the rewind is logged: {logged:?}"
    );
    assert_eq!(driver.engine().state_hash(), hash, "nothing was paid");
}

/// Both players pass: the top of the stack resolves.
pub fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.pass()?;
    driver.pass()?;
    Ok(())
}

/// Pays the pending payment by tapping lands automatically.
pub fn pay(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))
}

/// The labels of the ways to cast that are asked now.
pub fn ways(driver: &Driver) -> Result<Vec<String>, ScriptError> {
    let DecisionKind::ChooseOption(options) = &driver.decision()?.kind else {
        panic!("the way is asked: {}", driver.decision()?.kind.name());
    };
    Ok(options
        .options
        .iter()
        .map(|option| match option {
            ChoiceOption::Label(label) => label.clone(),
            other => panic!("a way is a label: {other:?}"),
        })
        .collect())
}

/// Answers the way to cast with the option whose label starts with `label`.
pub fn choose_way(driver: &mut Driver, label: &str) -> Result<(), ScriptError> {
    let at = ways(driver)?
        .iter()
        .position(|found| found.starts_with(label))
        .unwrap_or_else(|| panic!("no way {label}: {:?}", ways(driver)));
    driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
    Ok(())
}

/// Answers the question of a discard with the first `count` cards offered.
pub fn discard(driver: &mut Driver, count: usize) -> Result<(), ScriptError> {
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("the discard is asked: {:?}", driver.decision()?.kind.name());
    };
    let cards = options.from[..count].to_vec();
    driver.answer(Answer::ChooseCards(CardsChoice(cards)))?;
    Ok(())
}
