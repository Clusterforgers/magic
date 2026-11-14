//! Goal: with Library of Leng, each card an effect makes its controller discard may go on top of their library instead
//! of into the graveyard, asked card by card (the Library of Leng rulings), while a card discarded to pay a cost (cycling)
//! goes to the graveyard unasked; at two to four seats, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A table where seat 0 controls Library of Leng and two Islands and holds Mind Bomb, Backslide and two Grizzly Bears.
fn r6105_table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Library of Leng").unwrap()), ready(island()), ready(island())])
        .hand(
            0,
            [
                card("Mind Bomb").unwrap(),
                card("Backslide").unwrap(),
                card("Grizzly Bears").unwrap(),
                card("Grizzly Bears").unwrap(),
            ],
        )
}

/// The names of the cards in a zone, in its order.
fn r6105_names(driver: &Driver, zone: Zone) -> Vec<String> {
    driver
        .state()
        .zone(zone)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .map(|object| object.card().name.clone())
        .collect()
}

#[rstest]
fn r6105_an_effect_discard_may_go_on_top(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] first_on_top: bool,
) -> Result<(), ScriptError> {
    let scenario = r6105_table(seats);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let library_before = driver.state().zone(Zone::Library(pid(0))).len();
    let life_before = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Mind Bomb")?;
    let mut asked = 0;
    for _ in 0..80 {
        if driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options) if decision.player == pid(0) => {
                let bears: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| driver.state().object(*id).unwrap().card().name == "Grizzly Bears")
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(bears)))?;
            }
            DecisionKind::YesNo(_) => {
                assert_eq!(decision.player, pid(0), "the Library's controller is asked");
                let yes = if asked == 0 { first_on_top } else { !first_on_top };
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(asked, 2, "one question per card discarded");
    let library = driver.state().zone(Zone::Library(pid(0)));
    assert_eq!(library.len(), library_before + 1);
    assert_eq!(r6105_names(&driver, Zone::Graveyard(pid(0))).iter().filter(|name| *name == "Grizzly Bears").count(), 1);
    // Two cards discarded, wherever they went: Mind Bomb deals 1 to its caster.
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life_before - 1);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn r6105_a_cost_discard_goes_to_the_graveyard(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r6105_table(seats))?;
    driver.begin_activation("Backslide")?;
    for _ in 0..10 {
        match driver.decision()?.kind.clone() {
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::YesNo(_) => panic!("a cost is no effect"),
            _ => break,
        }
    }
    assert!(r6105_names(&driver, Zone::Graveyard(pid(0))).contains(&"Backslide".to_owned()));
    Ok(())
}
