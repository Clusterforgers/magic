//! Goal: "choose any number of creatures with different powers" plays at two to four seats: Sigardian Zealot's combat
//! trigger offers every creature, refuses two of one power, and pumps only the creatures chosen.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, giant};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Priority passes until a choice of cards is asked.
fn r4709_until_cards(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("the trigger never asked which creatures")
}

/// Seat 0 controls Sigardian Zealot (3/3), two Grizzly Bears (2/2) and a Craw Wurm (4/4); every other seat a bear.
#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r4709_zealot_pumps_creatures_of_different_powers(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(card("Sigardian Zealot").unwrap()),
            ready(bear()),
            ready(bear()),
            ready(giant()),
        ],
    );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    let mut driver = Driver::from_scenario(scenario.at(1, 0, Step::Main1))?;
    r4709_until_cards(&mut driver)?;
    let field: Vec<_> = driver.state().zone(Zone::Battlefield).to_vec();
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        unreachable!()
    };
    // Every creature is offered; powers 2, 3 and 4 make at most three.
    assert_eq!(options.from.len(), 4 + seats - 1);
    assert_eq!(options.max, 3);
    let (zealot, first, second, wurm) = (field[0], field[1], field[2], field[3]);
    assert!(
        driver
            .answer(Answer::ChooseCards(CardsChoice(vec![first, second])))
            .is_err()
    );
    driver.answer(Answer::ChooseCards(CardsChoice(vec![first, wurm])))?;
    let power = |id| characteristics(driver.state(), id).unwrap().power;
    assert_eq!(power(first), Some(5));
    assert_eq!(power(wurm), Some(7));
    assert_eq!(power(second), Some(2));
    assert_eq!(power(zealot), Some(3));
    assert!(has_keyword(driver.state(), first, &Keyword::Vigilance));
    assert!(!has_keyword(driver.state(), second, &Keyword::Vigilance));
    Ok(())
}
