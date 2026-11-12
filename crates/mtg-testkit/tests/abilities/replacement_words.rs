//! Goal: each effect of the order question carries the words of its own ability, with the source's name for `~`, whichever kind of replacement it is (a token doubler, a counter adder and doubler, a commander that would be drawn); a card is never quoted whole.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{name_of, pid};
use mtg_core::color::Color;
use mtg_engine::choose::ReplacementOptions;
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, plains};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::counter_cards::growth_spurt;
use mtg_testkit::replace_cards::raise_soldiers;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The (source name, words) of each effect of the first order question the game asks.
type Asked = Vec<(String, String)>;

const TOKENS: &str = "If an effect would create one or more tokens under your control, it creates twice that many of those tokens instead.";

/// Passes until the order question appears and returns what it offers.
fn first_question(driver: &mut Driver) -> Result<Asked, ScriptError> {
    for _ in 0..60 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseReplacement(options) => return Ok(offered(driver, &options)),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the order question".into()))
}

fn offered(driver: &Driver, options: &ReplacementOptions) -> Asked {
    options
        .candidates
        .iter()
        .map(|c| (name_of(driver.state(), c.source), c.text.clone()))
        .collect()
}

fn tokens() -> Result<Asked, ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [raise_soldiers().unwrap()])
            .battlefield(
                0,
                ["Anointed Procession", "Parallel Lives"]
                    .into_iter()
                    .map(|n| ready(card(n).unwrap()))
                    .chain([ready(plains())]),
            ),
    )?;
    driver.cast("Raise Soldiers")?;
    first_question(&mut driver)
}

fn counters() -> Result<Asked, ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [growth_spurt().unwrap()])
            .battlefield(
                0,
                ["Hardened Scales", "Doubling Season"]
                    .into_iter()
                    .map(|n| ready(card(n).unwrap()))
                    .chain([ready(forest()), ready(free_creature("Grizzly Bears", 2, 2))]),
            ),
    )?;
    let target = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Growth Spurt", &[target])?;
    first_question(&mut driver)
}

fn commander() -> Result<Asked, ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .format(Arc::new(Commander))
            .commanders(0, vec![legend("Cmd", 1, &[Color::Red], 2, 2)])
            .library(
                0,
                (0..10).map(|n| free_creature(&format!("Base {n}"), 1, 1)),
            )
            .battlefield(0, [ready(card("Thought Reflection").unwrap())]),
    )?;
    let object = driver
        .state()
        .zone(Zone::Command(pid(0)))
        .iter()
        .copied()
        .next()
        .unwrap();
    driver.manual(ManualAction::Move {
        object,
        to: ManualZone::LibraryTop,
    })?;
    // The draw stops at its first question, so the call may end short of a priority decision.
    let _ = driver.manual(ManualAction::Draw { count: 1 });
    first_question(&mut driver)
}

#[rstest]
#[case::tokens(
    tokens,
    &[("Anointed Procession", TOKENS), ("Parallel Lives", TOKENS)],
)]
#[case::counters(
    counters,
    &[
        ("Hardened Scales", "If one or more +1/+1 counters would be put on a creature you control, that many plus 1 +1/+1 counters are put on it instead."),
        ("Doubling Season", "If one or more counters would be put on a permanent you control, twice that many of those counters are put on it instead."),
    ],
)]
fn an_effect_carries_the_words_of_its_own_ability(
    #[case] play: fn() -> Result<Asked, ScriptError>,
    #[case] expected: &[(&str, &str)],
) {
    let asked = play().unwrap();
    let expected: Asked = expected
        .iter()
        .map(|(name, text)| ((*name).to_owned(), (*text).to_owned()))
        .collect();
    assert_eq!(asked, expected);
}

#[test]
fn a_commander_that_would_be_drawn_says_where_it_may_go_with_its_name() {
    let asked = commander().unwrap();
    let words = asked
        .iter()
        .find(|(name, _)| name == "Cmd")
        .map(|(_, text)| text.as_str());
    assert_eq!(
        words,
        Some(
            "If Cmd would be put into a hand or library, its owner may put it into the command zone instead."
        )
    );
    assert!(asked.iter().all(|(_, text)| !text.contains('~')));
}
