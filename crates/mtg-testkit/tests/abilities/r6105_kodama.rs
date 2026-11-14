//! Goal: Kodama of the East Tree lets its controller put a permanent card of equal or lesser mana value from their hand
//! onto the battlefield whenever another permanent they control enters, except one this ability put there; a card of a
//! greater mana value is not offered; at two to four seats, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, giant};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// How many permanents called `name` seat 0 controls.
fn r6105_mine(driver: &Driver, name: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == pid(0) && object.card().name == name)
        .count()
}

#[rstest]
fn r6105_kodama_puts_a_card_but_not_from_its_own_put(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] accept: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [ready(card("Kodama of the East Tree").unwrap()), ready(forest()), ready(forest())],
        )
        .hand(0, [bear(), bear(), bear(), giant()]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Grizzly Bears")?;
    let mut asked = 0;
    for _ in 0..80 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() && asked > 0 {
            break;
        }
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::YesNo(_) => {
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(accept)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let names: Vec<&str> = options
                    .from
                    .iter()
                    .map(|id| driver.state().object(*id).unwrap().card().name.as_str())
                    .collect();
                assert!(names.iter().all(|name| *name == "Grizzly Bears"), "no greater mana value: {names:?}");
                driver.answer(Answer::ChooseCards(CardsChoice(options.from[..1].to_vec())))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(asked, 1, "the card the ability put does not trigger it");
    assert!(driver.state().stack().is_empty());
    assert_eq!(r6105_mine(&driver, "Grizzly Bears"), if accept { 2 } else { 1 });
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), if accept { 2 } else { 3 });
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
