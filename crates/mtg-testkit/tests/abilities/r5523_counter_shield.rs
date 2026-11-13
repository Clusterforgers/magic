//! Goal: Prairie Dog's "{4}{W}: Until end of turn, if you would put one or more +1/+1 counters on a creature you
//! control, put that many plus one +1/+1 counters on it instead." adds one to the counters its controller puts later
//! that turn (its own end step trigger puts two instead of one), and without the activation one is put; at two to four
//! seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::plains;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

#[rstest]
fn r5523_prairie_dog_adds_one_counter(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] activated: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Prairie Dog").unwrap())];
    mine.extend((0..5).map(|_| ready(plains())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario)?;
    if activated {
        driver.begin_activation("Prairie Dog")?;
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        }
    }
    for _ in 0..300 {
        if driver.state().turn().active().index() != 0 {
            break;
        }
        driver.answer_passively()?;
    }
    let dog = driver.id_of("Prairie Dog")?;
    let counters = driver
        .state()
        .object(dog)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(counters, if activated { 2 } else { 1 });
    Ok(())
}
