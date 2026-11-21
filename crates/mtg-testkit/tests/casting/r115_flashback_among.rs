//! Goal: a flashback cost that removes X counters from among permanents with "X can't be 0" (CR 702.34a, 107.3, 601.2b)
//! is offered from the graveyard at two to four seats when the counters are there: Light Up the Night announces an X of
//! 1 up to the loyalty among its caster's planeswalkers, removes that many and deals X plus 1 to a creature; with no
//! planeswalker of its own (none, or only an opponent's) it is not offered from the graveyard.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::PresetPermanent;
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 has Light Up the Night in its graveyard and four Mountains; seat 1 has a Grizzly Bears; when `walker` is some,
/// Ajani Goldmane with 3 loyalty is that seat's.
fn r115_table(seats: usize, walker: Option<usize>) -> Scenario {
    let mine: Vec<PresetPermanent> = (0..4).map(|_| ready(mountain())).collect();
    let mut theirs = vec![ready(bear())];
    let ajani = PresetPermanent {
        counters: vec![(CounterKind::Loyalty, 3)],
        ..ready(card("Ajani Goldmane").unwrap())
    };
    let mut mine = mine;
    match walker {
        Some(0) => mine.push(ajani),
        Some(_) => theirs.push(ajani),
        None => {}
    }
    Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .graveyard(0, vec![card("Light Up the Night").unwrap()])
        .battlefield(0, mine)
        .battlefield(1, theirs)
}

/// Whether Light Up the Night is offered to seat 0 now.
fn r115_offered(driver: &Driver) -> Result<bool, ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        return Ok(false);
    };
    let state = driver.state();
    Ok(options.castable.iter().any(|id| {
        state
            .object(*id)
            .is_some_and(|object| object.card().name == "Light Up the Night")
    }))
}

#[rstest]
fn r115_light_up_the_night_flashes_back_for_loyalty(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] x: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r115_table(seats, Some(0)))?;
    assert!(r115_offered(&driver)?, "offered from the graveyard");
    let target = driver.target_named("Grizzly Bears")?;
    driver.begin_cast("Light Up the Night")?;
    let mut least = None;
    for _ in 0..16 {
        if !driver.state().stack().is_empty()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            break;
        }
        match &driver.decision()?.kind.clone() {
            DecisionKind::ChooseNumber(options) => {
                least = Some((options.min, options.max));
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let cards = options.from[..1].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(cards)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[target])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                let decision = driver.decision()?.clone();
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    assert_eq!(least, Some((1, Some(3))), "X is 1 up to the loyalty there");
    let ajani = driver.id_of("Ajani Goldmane")?;
    let left = driver
        .state()
        .object(ajani)
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&CounterKind::Loyalty));
    assert_eq!(left, Some(3 - x), "X loyalty counters removed");
    for _ in 0..8 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    let in_exile = driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == "Light Up the Night");
    assert!(in_exile, "a flashed-back spell is exiled");
    let bear_died = driver
        .state()
        .zone(Zone::Graveyard(pid(1)))
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == "Grizzly Bears");
    assert!(bear_died, "X plus 1 damage kills the 2/2");
    Ok(())
}

#[rstest]
#[case::no_planeswalker(None)]
#[case::an_opponents_planeswalker(Some(1))]
fn r115_without_loyalty_it_is_not_offered(
    #[values(2, 3, 4)] seats: usize,
    #[case] walker: Option<usize>,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(r115_table(seats, walker))?;
    assert!(!r115_offered(&driver)?, "X can't be 0");
    Ok(())
}
