//! Goal: cumulative upkeep of Jötun Grunt at 2 to 4 seats puts two cards from a single graveyard, any player's, on the
//! bottom of their owner's library for each age counter (CR 702.24): each payment takes its two cards from one owner, the
//! cards go under that owner's library, and the Grunt is sacrificed when no graveyard holds two cards.
#![allow(clippy::unwrap_used)]
use crate::common;

use common::{name_of, pid};
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls Jötun Grunt; seat 1's graveyard holds `theirs` cards and seat 0's holds `mine`.
fn grunt(seats: usize, mine: usize, theirs: usize) -> Result<Driver, ScriptError> {
    let card_of = |seat: usize, n: usize| free_creature(&format!("Dead {seat}-{n}"), 1, 1);
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Jötun Grunt").unwrap())])
        .graveyard(0, (0..mine).map(|n| card_of(0, n)))
        .graveyard(1, (0..theirs).map(|n| card_of(1, n)));
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 6]);
    }
    Driver::from_scenario(scenario)
}

/// Answers passively until a yes-or-no question is pending.
fn until_question(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the cumulative upkeep question".into()))
}

#[rstest]
#[case::their_graveyard(1, 3, true)]
#[case::my_graveyard(2, 1, true)]
#[case::one_card_each(1, 1, false)]
fn r113_jotun_grunt_bottoms_two_cards_of_one_graveyard(
    #[values(2, 3, 4)] seats: usize,
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] pays: bool,
) -> Result<(), ScriptError> {
    let mut driver = grunt(seats, mine, theirs)?;
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)?;
    until_question(&mut driver)?;
    let DecisionKind::YesNo(options) = &driver.decision()?.kind else {
        unreachable!("a question is pending");
    };
    assert_eq!(options.only_no, !pays, "{}", options.text);
    if !pays {
        driver.answer(Answer::YesNo(YesNoChoice(false)))?;
        while !driver.state().stack().is_empty() {
            driver.answer_passively()?;
        }
        assert!(driver.id_of("Jötun Grunt").is_err(), "it is sacrificed");
        return Ok(());
    }
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("the cards are picked");
    };
    assert_eq!(options.min, 2);
    let owner = usize::from(theirs >= 2);
    let other = 1 - owner;
    // A pick across two graveyards is refused.
    let across = vec![
        driver.state().zone(Zone::Graveyard(pid(owner)))[0],
        driver.state().zone(Zone::Graveyard(pid(other)))[0],
    ];
    assert!(
        driver
            .answer(Answer::ChooseCards(CardsChoice(across)))
            .is_err()
    );
    let picks: Vec<_> = driver.state().zone(Zone::Graveyard(pid(owner)))[..2].to_vec();
    driver.answer(Answer::ChooseCards(CardsChoice(picks.clone())))?;
    assert!(driver.id_of("Jötun Grunt").is_ok(), "it stays");
    let library = driver.state().zone(Zone::Library(pid(owner)));
    let bottom: Vec<String> = library[..2]
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    assert!(
        bottom
            .iter()
            .all(|name| name.starts_with(&format!("Dead {owner}-"))),
        "{bottom:?}"
    );
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(owner))).len(),
        if owner == 1 { theirs } else { mine } - 2
    );
    Ok(())
}

/// Every answer path of the bots takes the cards of a single graveyard: the random bot and the brain pick from one
/// owner, and a pick across graveyards is turned into one owner's by [`mtg_testkit::bot::apply_answer`].
#[rstest]
fn r112_every_bot_answer_takes_cards_of_one_graveyard(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    use mtg_testkit::bot::{RandomBot, apply_answer};
    use mtg_testkit::brain::{HeuristicBot, SeatBrain};
    let mut driver = grunt(seats, 2, 3)?;
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)?;
    until_question(&mut driver)?;
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    let decision = driver.decision()?.clone();
    assert!(matches!(decision.kind, DecisionKind::ChooseCards(_)));
    let engine = driver.engine();
    let across = vec![
        engine.state().zone(Zone::Graveyard(pid(0)))[0],
        engine.state().zone(Zone::Graveyard(pid(1)))[0],
    ];
    let mut fixed = engine.clone();
    apply_answer(
        &mut fixed,
        &decision,
        Answer::ChooseCards(CardsChoice(across)),
    )
    .unwrap();
    for seed in 0..24 {
        let answer = RandomBot::new(seed, 0).choose_in(&decision, engine.state());
        engine.clone().apply(answer.clone()).unwrap_or_else(|e| {
            panic!("random bot, seed {seed}: {answer:?}: {e}");
        });
        let settings = mtg_testkit::brain::sim::Settings::NORMAL.untimed();
        let answer = HeuristicBot::new(seed, pid(0), settings).choose(&decision, Some(engine));
        engine.clone().apply(answer.clone()).unwrap_or_else(|e| {
            panic!("brain, seed {seed}: {answer:?}: {e}");
        });
    }
    Ok(())
}
