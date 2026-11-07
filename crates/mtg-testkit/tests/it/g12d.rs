//! Goal: the engine pieces of pack g12d play at tables of two to four seats: a repeat marked `at_least_once` makes its first pass without reading its condition and then goes on while the condition holds, within its bound; a counted draw paid as an effect ("you may draw a card for each land you control; if you do, ...") draws as many cards as the count says when it resolves, or nothing when declined.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, spell};
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::Answer;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::swamp;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const LOW_LIFE: &str = r#"{"Compare":{"left":{"Life":"You"},"op":"Lt","right":30}}"#;

/// Lets the stack resolve, answering a yes-or-no question with `yes` and any other decision passively.
fn settle(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if driver.decision()?.kind.name() == "yes-no" {
            driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::once_though_already_false(LOW_LIFE, 1000, 40, 44)]
#[case::until_the_condition_fails(LOW_LIFE, 1000, 20, 32)]
#[case::bounded_by_one(r#""YourTurn""#, 1, 20, 24)]
#[case::bounded_by_two(r#""YourTurn""#, 2, 20, 28)]
fn a_do_while_repeat_makes_one_pass_first(
    #[case] condition: &str,
    #[case] most: u32,
    #[case] start: i32,
    #[case] end: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let effects = format!(
        r#"[{{"RepeatWhile":{{"condition":{condition},"effects":[{{"GainLife":{{"amount":4}}}}],"most":{most},"at_least_once":true}}}},{{"GainLife":{{"amount":1}}}}]"#
    );
    let trick = spell("Trick", CardType::Instant, "[]", &effects);
    let mut driver = Driver::from_scenario(Scenario::new(seats).life(0, start).hand(0, [trick]))?;
    driver.cast("Trick")?;
    settle(&mut driver, true)?;
    // The effect after the repeat runs once, after the last pass.
    driver.expect_life(0, end + 1)
}

#[rstest]
fn a_counted_draw_paid_as_an_effect_draws_the_count(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] lands: usize,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    let effects = r#"[{"Pay":{"cost":[{"Draw":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}],"paid":[{"GainLife":{"amount":5}}]}}]"#;
    let trick = spell("Trick", CardType::Instant, "[]", effects);
    let mut scenario = Scenario::new(seats)
        .hand(0, [trick])
        .library(0, (0..6).map(|_| swamp()))
        .battlefield(0, (0..lands).map(|_| ready(swamp())));
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Trick")?;
    settle(&mut driver, pays)?;
    let drawn = names(driver.state(), Zone::Hand(PlayerId(0))).len();
    assert_eq!(drawn, if pays { lands } else { 0 });
    driver.expect_life(0, if pays { 25 } else { 20 })
}
