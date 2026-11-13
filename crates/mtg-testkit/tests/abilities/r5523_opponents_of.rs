//! Goal: Heartwood Storyteller lets each opponent of the player who cast a noncreature spell draw a card, its
//! controller included when that player is its opponent, and never the caster; a creature spell is a near miss; at two to
//! four seats, each player choosing to draw.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn r5523_hand(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Hand(PlayerId(u64::try_from(seat).unwrap())))
        .len()
}

#[rstest]
#[case::noncreature("Glorious Anthem", true)]
#[case::near_miss_creature("Pup", false)]
fn r5523_storyteller_lets_the_casters_opponents_draw(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] draws: bool,
) -> Result<(), ScriptError> {
    let cast = if spell == "Pup" {
        free_creature("Pup", 1, 1)
    } else {
        card(spell).unwrap()
    };
    let mut scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(card("Heartwood Storyteller").unwrap())])
        .battlefield(1, (0..3).map(|_| ready(plains())))
        .hand(1, [cast]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| bear()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before: Vec<usize> = (0..seats).map(|seat| r5523_hand(&driver, seat)).collect();
    driver.begin_cast(spell)?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    for _ in 0..200 {
        let state = driver.state();
        if state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            break;
        }
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    for seat in 0..seats {
        let drew = r5523_hand(&driver, seat) + usize::from(seat == 1) - before[seat];
        let expected = usize::from(draws && seat != 1);
        assert_eq!(drew, expected, "seat {seat}");
    }
    Ok(())
}
