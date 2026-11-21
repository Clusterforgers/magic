//! Goal: "choose a color and an opponent" (`EntersChoice::R115ColorAndOpponent`) and "the chosen color is the most common color among nontoken permanents the chosen player controls but isn't tied for most common" (`Condition::R115MostCommonColor`, CR 105.1) play Call to Arms at tables of two to four seats: a color strictly most common among the chosen opponent's permanents gives white creatures +1/+1 and keeps the enchantment, a tied or smaller one sacrifices it at once, the permanents of the other opponents never count, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::Chosen;
use mtg_engine::zones::Zone;
use mtg_engine::{characteristics, replay};
use mtg_testkit::cards::{creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// Seat 0 holds Call to Arms and has a white 2/2 and plains; the last seat has one creature of each listed colour, and
/// every other opponent a crowd of blue creatures that must not count.
fn r115_table(seats: usize, colors: &[Color]) -> Scenario {
    let mut mine: Vec<_> = lands(4, plains);
    mine.push(ready(creature("White Knight", 1, &[Color::White], 2, 2)));
    let mut scenario = Scenario::new(seats)
        .hand(0, vec![card("Call to Arms").unwrap()])
        .battlefield(0, mine)
        .at(1, 0, mtg_engine::turn::Step::Main1);
    for seat in 1..seats {
        let board: Vec<_> = if seat == seats - 1 {
            colors
                .iter()
                .enumerate()
                .map(|(at, color)| ready(creature(&format!("Mark {at}"), 1, &[*color], 1, 1)))
                .collect()
        } else {
            (0..4)
                .map(|at| ready(creature(&format!("Crowd {at}"), 1, &[Color::Blue], 1, 1)))
                .collect()
        };
        scenario = scenario.battlefield(seat, board);
    }
    scenario
}

/// Passes until the stack is empty, choosing `color` and the last seat as they are offered.
fn r115_resolve(driver: &mut Driver, color: Color, chosen: usize) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| match option {
                        ChoiceOption::Color(offered) => *offered == color,
                        ChoiceOption::Player(player) => *player == pid(chosen),
                        _ => false,
                    })
                    .expect("the color and the opponent are offered");
                asked += 1;
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().zone(Zone::Stack).is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(asked);
            }
            _ => {
                driver.pass()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack does not empty".to_owned()))
}

#[rstest]
#[case::most_common(&[Color::Red, Color::Red, Color::Blue], Color::Red, true)]
#[case::a_lone_color(&[Color::Green], Color::Green, true)]
#[case::tied(&[Color::Red, Color::Blue], Color::Red, false)]
#[case::fewer(&[Color::Red, Color::Red, Color::Blue], Color::Blue, false)]
#[case::absent(&[Color::Red], Color::White, false)]
#[case::no_colors(&[], Color::Black, false)]
fn r115_call_to_arms_holds_only_while_the_color_leads(
    #[values(2, 3, 4)] seats: usize,
    #[case] colors: &[Color],
    #[case] color: Color,
    #[case] stays: bool,
) -> Result<(), ScriptError> {
    let chosen = seats - 1;
    let setup = r115_table(seats, colors).setup().unwrap();
    let mut driver = Driver::from_scenario(r115_table(seats, colors))?;
    driver.begin_cast("Call to Arms")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let asked = r115_resolve(&mut driver, color, chosen)?;
    let opponents = seats - 1;
    assert_eq!(
        asked,
        if opponents > 1 { 2 } else { 1 },
        "a lone opponent is no question"
    );
    let knight = driver.id_of("White Knight")?;
    let power = characteristics(driver.state(), knight).unwrap().power;
    let held = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|o| o.card().name == "Call to Arms")
        })
        .copied();
    if stays {
        let held = held.expect("Call to Arms stays");
        let remembered = driver
            .state()
            .object(held)
            .unwrap()
            .permanent()
            .unwrap()
            .chosen()
            .cloned();
        assert_eq!(
            remembered,
            Some(Chosen::R115ColorAndPlayer(color, pid(chosen)))
        );
        assert_eq!(power, Some(3), "white creatures get +1/+1");
    } else {
        assert!(held.is_none(), "Call to Arms is sacrificed");
        assert_eq!(power, Some(2));
        assert!(
            driver
                .state()
                .zone(Zone::Graveyard(pid(0)))
                .iter()
                .any(|id| driver
                    .state()
                    .object(*id)
                    .is_some_and(|o| o.card().name == "Call to Arms"))
        );
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
