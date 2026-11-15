//! Goal: Risky Move plays as printed at two to four seats: each upkeep its active player gains control of it, and the
//! new controller then flips a coin, giving a creature of theirs to the opponent they chose only when they lose; a
//! player who already controls it gains nothing and flips no coin.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::Event;
use mtg_engine::characteristics::controller_of;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, free_creature, island, keyworded};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// The controller of the only permanent of this name.
fn r6401_controller(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    let id = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == name)
        .unwrap();
    let controller = controller_of(state, id).unwrap();
    (0..4).find(|seat| pid(*seat) == controller).unwrap()
}

/// The coin flips of the game so far, won or lost.
fn r6401_flips(driver: &Driver) -> Vec<bool> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::CoinFlipped { won, .. } => Some(*won),
            _ => None,
        })
        .collect()
}

#[rstest]
fn r6401_risky_move_changes_hands_and_risks_a_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 4)] seed: u64,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .seed(seed)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Risky Move").unwrap())]);
    scenario = scenario.battlefield(1, [ready(bear())]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Ogre", 3, 3))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    // Seat 0 controls it already: its own upkeep would change nothing, and no coin was flipped.
    assert_eq!(r6401_flips(&driver).len(), 0);
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(r6401_controller(&driver, "Risky Move"), 1);
    let flips = r6401_flips(&driver);
    assert_eq!(flips.len(), 1, "one flip for one change of control");
    let bears = r6401_controller(&driver, "Grizzly Bears");
    if flips[0] {
        assert_eq!(bears, 1, "a won flip keeps the creature");
    } else {
        assert_ne!(bears, 1, "a lost flip gives the creature to an opponent");
    }
    Ok(())
}

/// The seat whose turn of the table is `steps` seats from `seat` (to the left when positive).
fn r6401_seat(seats: usize, seat: usize, steps: isize) -> usize {
    let n = isize::try_from(seats).unwrap();
    usize::try_from((isize::try_from(seat).unwrap() + steps).rem_euclid(n)).unwrap()
}

/// Answers until seat 0 has priority with an empty stack, choosing `left` or right when asked for a direction.
fn r6401_settle(driver: &mut Driver, left: bool) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if decision.player == pid(0)
                    && driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty()
                    && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseOption(options) if options.text.contains("left") => {
                driver.answer(Answer::ChooseOption(OptionChoice(usize::from(!left))))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("settle".into()))
}

/// Each seat's pet and a land, after `mine` for seat 0.
fn r6401_pets(
    mut scenario: Scenario,
    seats: usize,
    mine: &[mtg_engine::PresetPermanent],
) -> Scenario {
    for seat in 0..seats {
        let mut permanents = if seat == 0 { mine.to_vec() } else { Vec::new() };
        permanents.push(ready(free_creature(&format!("Pet {seat}"), 1, 1)));
        permanents.push(ready(forest()));
        scenario = scenario.battlefield(seat, permanents);
    }
    scenario
}

#[rstest]
fn r6401_aminatou_passes_every_nonland_permanent_in_the_chosen_direction(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] left: bool,
) -> Result<(), ScriptError> {
    let aminatou = with_counters(
        ready(card("Aminatou, the Fateshifter").unwrap()),
        CounterKind::Loyalty,
        7,
    );
    let scenario = r6401_pets(
        Scenario::new(seats).at(1, 0, Step::Main1),
        seats,
        &[aminatou],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Aminatou, the Fateshifter", 2)?;
    r6401_settle(&mut driver, left)?;
    for seat in 0..seats {
        // Each player takes from the next player in the direction, so a pet goes the other way.
        let taker = r6401_seat(seats, seat, if left { -1 } else { 1 });
        assert_eq!(r6401_controller(&driver, &format!("Pet {seat}")), taker);
    }
    assert_eq!(r6401_controller(&driver, "Aminatou, the Fateshifter"), 0);
    let lands_kept = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == "Forest")
        .all(|id| {
            let object = driver.state().object(*id).unwrap();
            controller_of(driver.state(), *id) == Some(object.owner())
        });
    assert!(lands_kept);
    Ok(())
}

#[rstest]
fn r6401_order_of_succession_passes_a_chosen_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] left: bool,
) -> Result<(), ScriptError> {
    let scenario = r6401_pets(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [card("Order of Succession").unwrap()]),
        seats,
        &(0..4).map(|_| ready(island())).collect::<Vec<_>>(),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Order of Succession")?;
    r6401_settle(&mut driver, left)?;
    for seat in 0..seats {
        let taker = r6401_seat(seats, seat, if left { -1 } else { 1 });
        assert_eq!(r6401_controller(&driver, &format!("Pet {seat}")), taker);
    }
    Ok(())
}

#[rstest]
fn r6401_inniaz_passes_a_permanent_from_the_right(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let flier = |name: &str| ready(keyworded(name, 1, 1, &[Keyword::Flying]));
    let scenario = r6401_pets(
        Scenario::new(seats).at(1, 0, Step::Main1),
        seats,
        &[
            ready(card("Inniaz, the Gale Force").unwrap()),
            flier("Kite"),
            flier("Gull"),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Inniaz, the Gale Force", 1), ("Kite", 1), ("Gull", 1)])?;
    r6401_settle(&mut driver, true)?;
    // Each player takes from the player to their right, so seat 0 takes from the last seat.
    for seat in 1..seats {
        let taker = r6401_seat(seats, seat, 1);
        assert_eq!(r6401_controller(&driver, &format!("Pet {seat}")), taker);
    }
    let given: usize = ["Inniaz, the Gale Force", "Kite", "Gull", "Pet 0"]
        .iter()
        .filter(|name| r6401_controller(&driver, name) == 1)
        .count();
    assert_eq!(given, 1, "seat 1 takes one nonland permanent of seat 0");
    Ok(())
}
