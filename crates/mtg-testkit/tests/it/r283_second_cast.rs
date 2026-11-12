//! Goal: "counter target spell that's the second spell cast this turn" (Second Guess) may target the second spell cast in
//! the turn, by whichever player, and neither the first one nor itself, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::removal::spell_named;
use common::{pid, spell};
use mtg_core::card::CardType;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::island;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn pass_until_priority(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..16 {
        if driver.decision()?.player == pid(seat) {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck(format!("priority for seat {seat}")))
}

#[rstest]
#[case::the_second_spell("Second", true)]
#[case::not_the_first("First", false)]
fn r283_second_guess_targets_only_the_second_spell_cast_this_turn(
    #[case] aimed: &str,
    #[case] legal: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let guesser = seats - 1;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(
                0,
                [
                    spell("First", CardType::Instant, "[]", "[]"),
                    spell(
                        "Second",
                        CardType::Instant,
                        "[]",
                        r#"[{"GainLife":{"amount":5}}]"#,
                    ),
                ],
            )
            .library(0, vec![island(); 4])
            .library(guesser, vec![island(); 4])
            .hand_named(guesser, ["Second Guess"])
            .battlefield(guesser, [ready(island()), ready(island())]),
    )?;
    driver.cast("First")?;
    driver.cast("Second")?;
    pass_until_priority(&mut driver, guesser)?;
    let target = spell_named(&driver, aimed);
    let cast = driver.cast_at("Second Guess", &[target]);
    assert_eq!(cast.is_ok(), legal, "{cast:?}");
    if !legal {
        return Ok(());
    }
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let graveyard = common::names(driver.state(), Zone::Graveyard(pid(0)));
    assert!(graveyard.contains(&"Second".to_owned()));
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, 20, "the second spell was countered");
    Ok(())
}
