//! Goal: a permanent that enters as a copy "except it enters with counters" (Spark Double, Undercover Operative, Altered Ego) gets the counters only when it is a copy and the copied permanent matches the entry, at 2 to 4 seats: +1/+1 on a copy of a creature and none of loyalty, a shield only when its controller controls the copied creature, X counters for the announced X, on a legend beside the original; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::replays;
use mtg_cards::value::CounterKind;
use mtg_core::card::Supertype;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, island};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds `spell` with six Islands and three Forests and controls a legendary Hero and a Squire; seat 1 controls a Brute.
fn table(seats: usize, spell: &str) -> Scenario {
    Scenario::new(seats)
        .hand(0, [card(spell).unwrap()])
        .battlefield(
            0,
            std::iter::once(ready(legend("Hero", 1, &[Color::Blue], 2, 2)))
                .chain(std::iter::once(ready(creature(
                    "Squire",
                    1,
                    &[Color::White],
                    1,
                    1,
                ))))
                .chain((0..6).map(|_| ready(island())))
                .chain((0..3).map(|_| ready(forest()))),
        )
        .battlefield(1, [ready(creature("Brute", 1, &[Color::Red], 3, 3))])
}

fn named(driver: &Driver, name: &str) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| state.object(*id).unwrap().card().name == name)
        .collect()
}

fn counters(driver: &Driver, id: ObjectId, kind: &CounterKind) -> u32 {
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(kind)
}

/// Casts `spell` with X = `x`, copies the permanent named `copy` (none when `None`) and settles the stack.
fn cast_copying(
    driver: &mut Driver,
    spell: &str,
    x: u32,
    copy: Option<&str>,
) -> Result<(), ScriptError> {
    driver.begin_cast(spell)?;
    for _ in 0..200 {
        let kind = driver.decision()?.kind.clone();
        match kind {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseCards(options) if options.text.contains("copy") => {
                let pick: Vec<ObjectId> = copy
                    .map(|name| named(driver, name))
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|id| options.from.contains(id))
                    .take(1)
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(pick)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the copy".into()))
}

#[rstest]
#[case::spark_double_on_a_creature("Spark Double", 0, Some("Hero"), "Hero", 1, 0)]
#[case::spark_double_on_a_plain_creature("Spark Double", 0, Some("Squire"), "Squire", 1, 0)]
#[case::operative_on_own_creature("Undercover Operative", 0, Some("Squire"), "Squire", 0, 1)]
#[case::operative_on_an_opponents_creature("Undercover Operative", 0, Some("Brute"), "Brute", 0, 0)]
#[case::altered_ego_x_two("Altered Ego", 2, Some("Brute"), "Brute", 2, 0)]
#[case::altered_ego_x_zero("Altered Ego", 0, Some("Brute"), "Brute", 0, 0)]
fn a_copy_enters_with_the_counters_its_entry_allows(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] x: u32,
    #[case] copy: Option<&str>,
    #[case] becomes: &str,
    #[case] plus: u32,
    #[case] shields: u32,
) -> Result<(), ScriptError> {
    let setup = table(seats, spell).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, spell))?;
    let before: Vec<ObjectId> = named(&driver, becomes);
    cast_copying(&mut driver, spell, x, copy)?;
    let entered: Vec<ObjectId> = named(&driver, becomes)
        .into_iter()
        .filter(|id| !before.contains(id))
        .collect();
    assert_eq!(entered.len(), 1, "one new {becomes}");
    let id = entered[0];
    assert_eq!(counters(&driver, id, &CounterKind::PlusOnePlusOne), plus);
    assert_eq!(
        counters(&driver, id, &CounterKind::Other("shield".into())),
        shields
    );
    assert_eq!(counters(&driver, id, &CounterKind::Loyalty), 0);
    // The originals keep what they had.
    for original in before {
        assert_eq!(counters(&driver, original, &CounterKind::PlusOnePlusOne), 0);
    }
    if spell == "Spark Double" && copy == Some("Hero") {
        // The copy is no legend, so both Heroes stay.
        let card = driver.state().object(id).unwrap().card();
        assert!(!card.supertypes.contains(&Supertype::Legendary));
        assert_eq!(named(&driver, "Hero").len(), 2);
    }
    replays(&driver, &setup);
    Ok(())
}
