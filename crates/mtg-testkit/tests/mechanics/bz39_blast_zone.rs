//! Goal: Blast Zone enters with a charge counter, its X ability adds X counters, and sacrificed as a cost it destroys
//! exactly the nonland permanents of every player, tokens included, whose mana value is the number of counters it had
//! (CR 608.2h last known information); at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::manual::{CommonToken, ManualAction, ManualToken};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const ZONE: &str = "Blast Zone";
/// The ability index of "{X}{X}, {T}: put X charge counters".
const ADD: usize = 1;
/// The ability index of "{3}, {T}, sacrifice: destroy".
const BLAST: usize = 2;

/// Nonland permanents of every seat, by mana value: 0, 1, 2, 3 and 12.
const BOARD: [(&str, u32); 5] = [
    ("Everflowing Chalice", 0),
    ("Llanowar Elves", 1),
    ("Grizzly Bears", 2),
    ("Arrogant Outlaw", 3),
    ("Ghalta, Primal Hunger", 12),
];

/// The tokens every seat gets, of mana value 0.
const TOKENS: [&str; 2] = ["Soldier", "Treasure"];

fn charge(driver: &Driver) -> u32 {
    let id = driver.id_of(ZONE).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Charge)
}

/// How many permanents called `name` are on the battlefield of `seat`.
fn on_battlefield(driver: &Driver, seat: usize, name: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .map(|id| driver.state().object(*id).unwrap())
        .filter(|object| object.controller() == pid(seat) && object.card().name == name)
        .count()
}

/// Activates the Blast Zone ability with this index, announces `x` when asked, pays automatically and
/// resolves the stack.
fn activate(driver: &mut Driver, ability: usize, x: u32) -> Result<(), ScriptError> {
    driver.begin_ability(ZONE, ability)?;
    loop {
        match &driver.decision()?.kind {
            DecisionKind::PayMana(_) => break,
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    panic!("the stack does not empty");
}

#[rstest]
fn blast_zone_destroys_each_nonland_permanent_with_mana_value_equal_to_its_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3, 12)] counters: u32,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(3, 0, Step::Main1);
    for seat in 0..seats {
        let mut permanents: Vec<_> = BOARD
            .iter()
            .map(|(name, _)| ready(card(name).unwrap()))
            .collect();
        permanents.extend((0..3).map(|_| ready(mountain())));
        if seat == 0 {
            permanents.push(with_counters(
                ready(card(ZONE).unwrap()),
                CounterKind::Charge,
                counters,
            ));
        }
        scenario = scenario.battlefield(seat, permanents);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    for seat in 0..seats {
        for token in [CommonToken::Soldier, CommonToken::Treasure] {
            driver.manual_by(
                pid(seat),
                &ManualAction::CreateToken {
                    token: ManualToken::Common(token),
                    count: 1,
                },
            )?;
        }
    }
    assert_eq!(charge(&driver), counters);
    activate(&mut driver, BLAST, 0)?;
    assert_eq!(on_battlefield(&driver, 0, ZONE), 0, "sacrificed");
    for seat in 0..seats {
        for (name, value) in BOARD {
            let left = usize::from(value != counters);
            assert_eq!(
                on_battlefield(&driver, seat, name),
                left,
                "{name} of seat {seat}"
            );
        }
        for token in TOKENS {
            let left = usize::from(counters != 0);
            assert_eq!(
                on_battlefield(&driver, seat, token),
                left,
                "{token} of seat {seat}"
            );
        }
        assert_eq!(on_battlefield(&driver, seat, "Mountain"), 3, "lands stay");
    }
    Ok(())
}

#[rstest]
fn blast_zone_puts_x_charge_counters_on_itself(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2)] x: u32,
) -> Result<(), ScriptError> {
    let mut permanents: Vec<_> = (0..4).map(|_| ready(mountain())).collect();
    permanents.push(with_counters(
        ready(card(ZONE).unwrap()),
        CounterKind::Charge,
        1,
    ));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(3, 0, Step::Main1)
            .battlefield(0, permanents),
    )?;
    activate(&mut driver, ADD, x)?;
    assert_eq!(charge(&driver), 1 + x);
    Ok(())
}

#[rstest]
fn blast_zone_enters_with_a_charge_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(3, 0, Step::Main1)
            .hand(0, vec![card(ZONE).unwrap()]),
    )?;
    driver.play_land(ZONE)?;
    assert_eq!(charge(&driver), 1);
    Ok(())
}
