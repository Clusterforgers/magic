//! Goal: "whenever you or a permanent you control becomes the target of a spell or ability an opponent controls" plays
//! as Leovold and Unsettled Mariner read at tables of two to four seats: it triggers once for the player and once for
//! each permanent of theirs an opponent's ability targets, never for another player's permanent or their own ability,
//! and "that spell or ability" and "its controller" name what targeted them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// An artifact named `name` with "{0}: Choose `count` target permanents or players. You gain 1 life."
fn pointer(name: &str, what: &str, count: u32) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Activated":{{"cost":[],"targets":[{{"what":{what},"bounds":{{"min":{count},"max":{count}}}}}],"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

/// A land of no rules, a permanent to be targeted.
fn rock(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Land))
}

/// Answers passively, but yes to every "you may" when `may` is set, until nothing is on the stack and nothing waits.
fn settle(driver: &mut Driver, may: bool) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        if may && matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

/// The targets by name: a seat number is that player, any other word the permanent of that name.
fn targets(driver: &Driver, names: &[&str]) -> Result<Vec<Target>, ScriptError> {
    names
        .iter()
        .map(|name| match name.parse::<usize>() {
            Ok(seat) => Ok(driver.target_player(seat)),
            Err(_) => driver.target_named(name),
        })
        .collect()
}

/// Seat 1 controls Leovold and the Mine rock, seat 0 the Theirs rock; `by` activates a pointer at `chosen`.
#[rstest]
#[case::the_player(0, r#""Player""#, &["1"], 1)]
#[case::a_permanent(0, r#"{"Object":[]}"#, &["Mine"], 1)]
#[case::leovold_itself(0, r#"{"Object":[]}"#, &["Leovold, Emissary of Trest"], 1)]
#[case::two_permanents(0, r#"{"Object":[]}"#, &["Mine", "Leovold, Emissary of Trest"], 2)]
#[case::another_players_permanent(0, r#"{"Object":[]}"#, &["Theirs"], 0)]
#[case::another_player(0, r#""Player""#, &["0"], 0)]
#[case::by_its_controller(1, r#"{"Object":[]}"#, &["Mine"], 0)]
fn leovold_draws_for_each_target_an_opponent_chooses(
    #[case] by: usize,
    #[case] what: &str,
    #[case] chosen: &[&str],
    #[case] draws: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let count = u32::try_from(chosen.len()).unwrap();
    let mut zero = vec![ready(rock("Theirs"))];
    let mut one = vec![
        ready(card("Leovold, Emissary of Trest").unwrap()),
        ready(rock("Mine")),
    ];
    if by == 0 { &mut zero } else { &mut one }.push(ready(pointer("Pointer", what, count)));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .library(1, (0..5).map(|n| rock(&format!("Card {n}"))))
            .battlefield(0, zero)
            .battlefield(1, one),
    )?;
    let before = hand(&driver, 1);
    if by == 1 {
        driver.pass()?;
    }
    driver.begin_ability("Pointer", 0)?;
    let chosen = targets(&driver, chosen)?;
    driver.choose_targets(&chosen)?;
    settle(&mut driver, true)?;
    assert_eq!(hand(&driver, 1) - before, draws);
    Ok(())
}

/// Seat 1 controls the Mariner; seat 0 has no mana, so what targets seat 1 or the Mariner is countered.
#[rstest]
#[case::the_player(r#""Player""#, "1", 0)]
#[case::the_mariner(r#"{"Object":[]}"#, "Unsettled Mariner", 0)]
#[case::its_own_rock(r#"{"Object":[]}"#, "Theirs", 1)]
fn unsettled_mariner_counters_what_targets_you_or_yours_unless_paid(
    #[case] what: &str,
    #[case] chosen: &str,
    #[case] gained: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                [ready(pointer("Pointer", what, 1)), ready(rock("Theirs"))],
            )
            .battlefield(1, [ready(card("Unsettled Mariner").unwrap())]),
    )?;
    let before = driver.state().player(pid(0)).unwrap().life();
    driver.begin_ability("Pointer", 0)?;
    let chosen = targets(&driver, &[chosen])?;
    driver.choose_targets(&chosen)?;
    settle(&mut driver, false)?;
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life() - before,
        gained
    );
    Ok(())
}
