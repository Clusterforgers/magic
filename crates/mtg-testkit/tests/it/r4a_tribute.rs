//! Goal: tribute (CR 702.104) asks an opponent the controller picks whether to put the counters on the creature as it enters, "if tribute wasn't paid" triggers only when they decline, and an opponent who does not answer declines; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

const TRIBUTE: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"Tribute":2}}}"#;
const UNPAID: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Compare":{"left":"Chosen","op":"Eq","right":0}},"effects":[{"GainLife":{"amount":3}}]}}"#;

fn bones() -> Arc<CardDefinition> {
    let card = (*free_creature("Bones", 3, 1)).clone();
    Arc::new(
        card.with_rules(serde_json::from_str::<Ability>(TRIBUTE).unwrap())
            .with_rules(serde_json::from_str::<Ability>(UNPAID).unwrap()),
    )
}

fn table(seats: usize) -> Driver {
    let mut scenario = Scenario::new(seats).hand(0, vec![bones()]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Casts the creature and answers the questions: the opponent pick (the first) and the tribute (yes or no, or no answer at all).
fn enter(driver: &mut Driver, pays: Option<bool>) {
    driver.cast("Bones").unwrap();
    for _ in 0..24 {
        match &driver.decision().unwrap().kind {
            DecisionKind::ChooseOption(_) => {
                assert_eq!(driver.decision().unwrap().player.0, 0);
                driver
                    .answer(Answer::ChooseOption(OptionChoice(0)))
                    .unwrap();
            }
            DecisionKind::YesNo(_) => {
                // Only an opponent is asked about the tribute.
                assert_ne!(driver.decision().unwrap().player.0, 0);
                match pays {
                    Some(yes) => driver.answer(Answer::YesNo(YesNoChoice(yes))).unwrap(),
                    None => driver.answer_passively().unwrap(),
                };
            }
            _ if driver.state().stack().is_empty() => return,
            _ => {
                driver.pass().unwrap();
            }
        }
    }
    panic!("the creature never settled");
}

fn counters(driver: &Driver) -> u32 {
    let id = driver.id_of("Bones").unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .filter(|(kind, _)| **kind == CounterKind::PlusOnePlusOne)
        .map(|(_, n)| n)
        .sum()
}

fn life(driver: &Driver) -> i32 {
    driver
        .state()
        .player(driver.state().turn().active())
        .unwrap()
        .life()
}

#[rstest]
fn a_paid_tribute_gives_the_counters_and_no_trigger(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats);
    let before = life(&driver);
    enter(&mut driver, Some(true));
    assert_eq!(counters(&driver), 2);
    assert_eq!(life(&driver), before);
}

#[rstest]
fn a_declined_tribute_triggers(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats);
    let before = life(&driver);
    enter(&mut driver, Some(false));
    assert_eq!(counters(&driver), 0);
    assert_eq!(life(&driver), before + 3);
}

#[rstest]
fn an_unanswered_tribute_is_declined(#[values(2, 3)] seats: usize) {
    let mut driver = table(seats);
    let before = life(&driver);
    enter(&mut driver, None);
    assert_eq!(counters(&driver), 0);
    assert_eq!(life(&driver), before + 3);
}
