//! Goal: a repeat marked `at_least_once` makes its first pass without reading its condition and then goes on while the condition holds, within its bound, at tables of two to four seats.

use crate::common::spell;
use mtg_core::card::CardType;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

const LOW_LIFE: &str = r#"{"Compare":{"left":{"Life":"You"},"op":"Lt","right":30}}"#;

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
    while !driver.state().stack().is_empty() && driver.state().result().is_none() {
        driver.pass()?;
    }
    // The effect after the repeat runs once, after the last pass.
    driver.expect_life(0, end + 1)
}
