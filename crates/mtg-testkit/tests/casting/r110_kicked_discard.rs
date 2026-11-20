//! Goal: a kicker whose cost discards a card is read back by the permanent at 2 to 4 seats: Dralnu's Pet kicked by
//! discarding a creature card enters with flying and as many +1/+1 counters as that card's mana value, and cast without
//! its kicker enters with neither.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, CostsChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 holds Dralnu's Pet and `discard`, with three Islands and three Swamps, in its first main phase.
fn game(seats: usize, discard: &str) -> Result<Driver, ScriptError> {
    let lands = vec![
        ready(island()),
        ready(island()),
        ready(island()),
        ready(swamp()),
        ready(swamp()),
        ready(swamp()),
    ];
    let mut scenario = Scenario::new(seats)
        .hand(
            0,
            vec![card("Dralnu's Pet").unwrap(), card(discard).unwrap()],
        )
        .battlefield(0, lands);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    Driver::from_scenario(scenario)
}

/// Casts Dralnu's Pet, kicked or not, answering the discard with the first card offered, and resolves it.
fn cast(driver: &mut Driver, kicked: bool) -> Result<(), ScriptError> {
    driver.begin_cast("Dralnu's Pet")?;
    for _ in 0..30 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseCosts(_) => {
                let costs = if kicked { vec![0] } else { Vec::new() };
                driver.answer(Answer::ChooseCosts(CostsChoice::Costs(costs)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let chosen = options.from.iter().copied().take(1).collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_)
                if driver.state().zone(Zone::Stack).is_empty()
                    && driver.state().zone(Zone::Battlefield).iter().any(|id| {
                        crate::common::name_of(driver.state(), *id) == "Dralnu's Pet"
                    }) =>
            {
                return Ok(());
            }
            _ => {
                driver.pass()?;
            }
        }
    }
    Err(ScriptError::Stuck("the cast of Dralnu's Pet".into()))
}

#[rstest]
#[case::kicked_by_an_angel("Serra Angel", true, 5)]
#[case::kicked_by_a_bear("Grizzly Bears", true, 2)]
#[case::not_kicked("Serra Angel", false, 0)]
fn the_pet_reads_the_discarded_card(
    #[values(2, 3, 4)] seats: usize,
    #[case] discard: &str,
    #[case] kicked: bool,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, discard)?;
    cast(&mut driver, kicked)?;
    let pet = driver.id_of("Dralnu's Pet")?;
    let held = driver
        .state()
        .object(pet)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    assert_eq!(held, counters);
    assert_eq!(
        mtg_engine::keywords::has_keyword(driver.state(), pet, &Keyword::Flying),
        kicked
    );
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(0))).len(),
        usize::from(kicked),
        "the kicker discarded the card"
    );
    Ok(())
}
