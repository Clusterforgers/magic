//! Goal: Sylvan Library at the beginning of its controller's draw step may draw two more cards; then they choose two
//! cards in their hand drawn this turn, never one that was there before, and for each pay 4 life or put it back on top of
//! the library; declining draws nothing more and asks nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// Seat 0 controls Sylvan Library, holds Old Card and has ten named cards in the library; it is their upkeep.
fn table() -> Scenario {
    Scenario::new(2)
        .at(2, 0, Step::Upkeep)
        .battlefield(0, [ready(card("Sylvan Library").unwrap())])
        .hand(0, [free_creature("Old Card", 1, 1)])
        .library(
            0,
            (0..10).map(|at| free_creature(&format!("Card {at}"), 1, 1)),
        )
        .library(1, (0..10).map(|_| forest()))
}

/// Plays the draw step: `draw` to the "may", and `pay` to each payment of life. The cards offered to choose from.
fn play(driver: &mut Driver, draw: bool, pay: bool) -> Result<Vec<String>, ScriptError> {
    let mut offered = Vec::new();
    driver.pass_until(Step::Draw)?;
    let mut asked_draw = false;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
            && asked_draw
        {
            return Ok(offered);
        }
        let answer = match &decision.kind {
            DecisionKind::YesNo(o) if !asked_draw => {
                asked_draw = true;
                Answer::YesNo(YesNoChoice(draw && !o.only_no))
            }
            DecisionKind::YesNo(o) => Answer::YesNo(YesNoChoice(pay && !o.only_no)),
            DecisionKind::ChooseCards(o) => {
                offered = o
                    .from
                    .iter()
                    .map(|id| crate::common::name_of(driver.state(), *id))
                    .collect();
                offered.sort();
                let take = usize::try_from(o.max).unwrap();
                Answer::ChooseCards(CardsChoice(o.from.iter().copied().take(take).collect()))
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
                continue;
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the draw step".into()))
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

#[rstest]
#[case::pays_for_both(true, true, 4, 12, 7)]
#[case::puts_both_back(true, false, 2, 20, 9)]
#[case::declines(false, false, 2, 20, 9)]
fn sylvan_library_draws_two_and_charges_for_those_kept(
    #[case] draw: bool,
    #[case] pay: bool,
    #[case] hand: usize,
    #[case] life_after: i32,
    #[case] library: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table())?;
    let offered = play(&mut driver, draw, pay)?;
    let held = names(driver.state(), Zone::Hand(pid(0)));
    assert_eq!(held.len(), hand, "{held:?}");
    assert!(held.contains(&"Old Card".to_owned()));
    assert_eq!(life(&driver), life_after);
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), library);
    if draw {
        assert_eq!(
            offered.len(),
            3,
            "the three cards drawn this turn: {offered:?}"
        );
        assert!(!offered.contains(&"Old Card".to_owned()));
    } else {
        assert_eq!(offered, Vec::<String>::new());
    }
    Ok(())
}
