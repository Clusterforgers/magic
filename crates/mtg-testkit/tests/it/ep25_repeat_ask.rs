//! Goal: a repeat that asks ("You may repeat this process any number of times") makes its first pass unasked when it is a do-while and asks before every pass otherwise, goes on with each yes and stops at the first no, never passes its bound, and asks the players it names in turn order until one says yes, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::spell;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::Answer;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// Casts a spell whose effects are a repeat that gains 1 life each pass, asking `ask`, and answers its questions: a
/// question is answered yes by `says_yes(player, round)`, where `round` counts the passes asked about so far. Returns the
/// players asked, in order, and the life seat 0 gained.
fn play(
    seats: usize,
    ask: &str,
    at_least_once: bool,
    most: u32,
    says_yes: impl Fn(PlayerId, usize) -> bool,
) -> Result<(Vec<PlayerId>, i32), ScriptError> {
    let effects = format!(
        r#"[{{"RepeatWhile":{{"condition":{{"Compare":{{"left":0,"op":"Eq","right":0}}}},"effects":[{{"GainLife":{{"amount":1}}}}],"most":{most},"at_least_once":{at_least_once},"ask":{ask}}}}}]"#
    );
    let trick = spell("Trick", CardType::Instant, "[]", &effects);
    let mut driver = Driver::from_scenario(Scenario::new(seats).hand(0, [trick]))?;
    driver.cast("Trick")?;
    let mut asked = Vec::new();
    let mut round = 0;
    for _ in 0..400 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            let life = driver.state().player(PlayerId(0)).unwrap().life();
            return Ok((asked, life - 20));
        }
        let decision = driver.decision()?;
        if decision.kind.name() == "yes-no" {
            let player = decision.player;
            asked.push(player);
            let yes = says_yes(player, round);
            if yes || player == PlayerId(u64::try_from(seats).unwrap() - 1) {
                round += 1;
            }
            driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::do_while_stopped_at_once(true, 1000, 0, 1, 1)]
#[case::do_while_twice_more(true, 1000, 2, 3, 3)]
#[case::asked_before_the_first(false, 1000, 0, 0, 1)]
#[case::asked_three_yes(false, 1000, 3, 3, 4)]
#[case::bounded(true, 2, 5, 2, 1)]
fn a_repeat_that_asks_goes_on_with_each_yes(
    #[case] at_least_once: bool,
    #[case] most: u32,
    #[case] yeses: usize,
    #[case] passes: i32,
    #[case] questions: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (asked, gained) = play(seats, r#""You""#, at_least_once, most, |_, round| {
        round < yeses
    })?;
    assert_eq!(gained, passes);
    assert_eq!(asked.len(), questions, "{asked:?}");
    assert!(asked.iter().all(|player| *player == PlayerId(0)));
    Ok(())
}

#[rstest]
fn the_players_it_names_are_asked_in_turn_until_one_says_yes(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2)] yeses: usize,
) -> Result<(), ScriptError> {
    let last = PlayerId(u64::try_from(seats).unwrap() - 1);
    let (asked, gained) = play(seats, r#""Opponents""#, true, 1000, |player, round| {
        player == last && round < yeses
    })?;
    assert_eq!(gained, i32::try_from(yeses).unwrap() + 1);
    let opponents: Vec<PlayerId> = (1..seats as u64).map(PlayerId).collect();
    // Each round asks every opponent in turn order: only the last one says yes.
    assert_eq!(asked.len(), (yeses + 1) * opponents.len());
    for (at, player) in asked.iter().enumerate() {
        assert_eq!(*player, opponents[at % opponents.len()]);
    }
    Ok(())
}
