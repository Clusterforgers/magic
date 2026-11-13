//! Goal: Iron Mastiff rolls one d20 for each player being attacked and only the highest roll counts, at two to four
//! seats: the number of dice is the number of attacked players, and the damage goes where the highest roll's row says.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Priority passes until the stack is empty, keeping the events.
fn r5518_settle(driver: &mut Driver, log: &mut Vec<Event>) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        log.extend(driver.pass()?);
    }
    Ok(())
}

#[rstest]
#[case::two_seats_one_attacked(2, 1, 1)]
#[case::three_seats_one_attacked(3, 1, 3)]
#[case::three_seats_two_attacked(3, 2, 5)]
#[case::four_seats_three_attacked(4, 3, 7)]
#[case::four_seats_two_attacked(4, 2, 11)]
fn r5518_iron_mastiff_rolls_for_each_attacked_player_and_keeps_the_highest(
    #[case] seats: usize,
    #[case] attacked: usize,
    #[case] seed: u64,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Iron Mastiff").unwrap())];
    let helpers: Vec<String> = (2..=attacked).map(|seat| format!("Hound {seat}")).collect();
    for name in &helpers {
        mine.push(ready(free_creature(name, 1, 1)));
    }
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, mine)
            .seed(seed)
            .at(1, 0, Step::Main1),
    )?;
    driver.pass_until(Step::DeclareAttackers)?;
    let mut attacks = vec![("Iron Mastiff", 1)];
    for (at, name) in helpers.iter().enumerate() {
        attacks.push((name.as_str(), at + 2));
    }
    let mut log = driver.attack(&attacks)?;
    r5518_settle(&mut driver, &mut log)?;
    let rolls: Vec<u32> = log
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled { result, sides: 20, .. } => Some(*result),
            _ => None,
        })
        .collect();
    assert_eq!(rolls.len(), attacked, "one d20 per attacked player: {rolls:?}");
    let highest = *rolls.iter().max().unwrap();
    let life = |seat: usize| driver.state().player(PlayerId::from_index(seat)).unwrap().life();
    // The Mastiff is a 4/4 and combat damage has not been dealt yet.
    let lost: Vec<i32> = (0..seats).map(|seat| 20 - life(seat)).collect();
    let mut expected = vec![0; seats];
    match highest {
        1..=9 => expected[0] = 4,
        10..=19 => expected[1] = 4,
        _ => expected.iter_mut().skip(1).for_each(|lost| *lost = 4),
    }
    assert_eq!(lost, expected, "highest roll {highest} of {rolls:?}");
    Ok(())
}
