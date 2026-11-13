//! Goal: "you may pay one or more {E}. If you do, create an X/X ... Vehicle ..., where X is the amount of {E} paid this
//! way." (Pia Nalaar, Chief Mechanic) plays at two to four seats: with no energy the payment offers only No and no token
//! comes; with energy the payer chooses X from 1 to what they hold, never 0, and the Vehicle is X/X.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::Answer;
use mtg_engine::choose::{NumberChoice, YesNoChoice};
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn r4707_energy(driver: &Driver) -> u32 {
    driver
        .state()
        .player(pid(0))
        .unwrap()
        .counters_of(&CounterKind::Other("energy".into()))
}

#[rstest]
#[case::no_energy(0, None)]
#[case::one(1, Some(1))]
#[case::three_of_three(3, Some(3))]
fn r4707_pia_pays_one_or_more_energy(
    #[case] held: u32,
    #[case] paid: Option<u32>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main2)
        .battlefield(0, [ready(card("Pia Nalaar, Chief Mechanic").unwrap())])
        .counters(0, CounterKind::Other("energy".into()), held);
    let mut driver = Driver::from_scenario(scenario)?;
    let mut asked = false;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::YesNo(options) => {
                asked = true;
                assert_eq!(
                    options.only_no,
                    paid.is_none(),
                    "No alone with {held} energy"
                );
                Answer::YesNo(YesNoChoice(paid.is_some()))
            }
            DecisionKind::ChooseNumber(options) => {
                assert_eq!(options.min, 1, "X is never 0");
                assert_eq!(options.max, Some(held));
                Answer::ChooseNumber(NumberChoice::Number(paid.unwrap()))
            }
            _ if asked && driver.state().stack().is_empty() => break,
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    assert!(asked, "the end step trigger asks for the payment");
    let jets: Vec<_> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.card().name == "Nalaar Aetherjet")
        .map(|o| (o.card().power, o.card().toughness))
        .collect();
    match paid {
        None => assert_eq!(jets, []),
        Some(x) => {
            let x = i32::try_from(x).unwrap();
            assert_eq!(jets, vec![(Some(x), Some(x))]);
        }
    }
    assert_eq!(r4707_energy(&driver), held - paid.unwrap_or(0));
    Ok(())
}
