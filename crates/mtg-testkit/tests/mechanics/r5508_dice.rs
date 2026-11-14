//! Goal: the dice a player rolls are counted for the turn at two to four seats: Resolute Veggiesaur gets its counter for
//! its controller's third die of the turn only (two dice of one roll count as two), The Space Family Goblinson gets a
//! counter for each die its controller rolls and has trample from the third on, and another player's dice count for
//! neither.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An artifact with "{0}: Roll `dice` six-sided dice."
fn r5508_roller(dice: u32) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{0}}","effects":[{{"RollDice":{{"sides":6,"dice":{dice},"results":[]}}}}]}}}}"#
    );
    let roll: Ability = serde_json::from_str(&json).unwrap();
    Arc::new(
        CardDefinition::new("Die Roller")
            .with_type(CardType::Artifact)
            .with_rules(roll),
    )
}

/// Answers passively until the stack and the waiting triggers are empty.
fn r5508_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let state = driver.state();
        if state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 controls Resolute Veggiesaur and The Space Family Goblinson; seat `roller` controls a Die Roller rolling
/// `dice` dice; it is `roller`'s first main phase.
fn r5508_table(seats: usize, roller: usize, dice: u32) -> Result<Driver, ScriptError> {
    let mut mine = vec![
        ready(card("Resolute Veggiesaur").unwrap()),
        ready(card("The Space Family Goblinson").unwrap()),
    ];
    let mut scenario = Scenario::new(seats).seed(7).at(2, roller, Step::Main1);
    if roller == 0 {
        mine.push(ready(r5508_roller(dice)));
    } else {
        scenario = scenario.battlefield(roller, [ready(r5508_roller(dice))]);
    }
    Driver::from_scenario(scenario.battlefield(0, mine))
}

/// Activates the Die Roller `times` times.
fn r5508_roll(driver: &mut Driver, times: usize) -> Result<(), ScriptError> {
    for _ in 0..times {
        driver.begin_activation("Die Roller")?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        r5508_settle(driver)?;
    }
    Ok(())
}

/// The +1/+1 counters on seat 0's permanent of that name.
fn r5508_counters(driver: &Driver, name: &str) -> u32 {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|object| object.card().name == name && object.controller() == PlayerId(0))
        .and_then(|object| {
            object
                .permanent()
                .map(|p| p.counters_of(&CounterKind::PlusOnePlusOne))
        })
        .unwrap()
}

fn r5508_tramples(driver: &Driver) -> bool {
    let id = driver.id_of("The Space Family Goblinson").unwrap();
    has_keyword(driver.state(), id, &Keyword::Trample)
}

#[rstest]
#[case::two_dice_two_seats(2, 2, 1, 0, false)]
#[case::three_dice_three_seats(3, 3, 1, 1, true)]
#[case::five_dice_four_seats(4, 5, 1, 1, true)]
#[case::two_pairs_two_seats(2, 2, 2, 1, true)]
#[case::one_pair_four_seats(4, 1, 2, 0, false)]
fn r5508_the_third_die_of_the_turn_counts(
    #[case] seats: usize,
    #[case] rolls: usize,
    #[case] dice: u32,
    #[case] veggiesaur: u32,
    #[case] trample: bool,
) -> Result<(), ScriptError> {
    let mut driver = r5508_table(seats, 0, dice)?;
    r5508_roll(&mut driver, rolls)?;
    let rolled = u32::try_from(rolls).unwrap() * dice;
    assert_eq!(r5508_counters(&driver, "Resolute Veggiesaur"), veggiesaur);
    assert_eq!(
        r5508_counters(&driver, "The Space Family Goblinson"),
        rolled
    );
    assert_eq!(r5508_tramples(&driver), trample);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn r5508_another_players_dice_count_for_neither(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut driver = r5508_table(seats, 1, 2)?;
    r5508_roll(&mut driver, 2)?;
    assert_eq!(r5508_counters(&driver, "Resolute Veggiesaur"), 0);
    assert_eq!(r5508_counters(&driver, "The Space Family Goblinson"), 0);
    assert!(!r5508_tramples(&driver));
    Ok(())
}
