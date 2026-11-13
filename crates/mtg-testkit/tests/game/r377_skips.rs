//! Goal: "skips all combat phases of their next turn" (False Peace, Empty City Ruse) skips every combat step of the
//! target's next turn only, whether the target is the caster or an opponent, at two to four seats, and Ral Zarek's −7
//! skips as many of the target's turns as its coins came up heads; each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::CoinSide;
use mtg_engine::events::Event;
use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::{Phase, Step};
use mtg_testkit::cards::plains;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

/// The log of the driver replays to its state.
fn replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Each turn begun in the log, with its active seat and whether a combat step began in it.
fn turns(driver: &Driver) -> Vec<(u32, usize, bool)> {
    let mut turns: Vec<(u32, usize, bool)> = Vec::new();
    for event in driver.engine().events() {
        match event {
            Event::TurnBegan { number, active, .. } => {
                turns.push((*number, active.index(), false));
            }
            Event::StepBegan { step } if step.phase() == Phase::Combat => {
                if let Some(last) = turns.last_mut() {
                    last.2 = true;
                }
            }
            _ => {}
        }
    }
    turns
}

#[rstest]
#[case::two_at_the_opponent(2, "False Peace", 1)]
#[case::three_at_the_opponent(3, "False Peace", 1)]
#[case::four_at_the_last(4, "False Peace", 3)]
#[case::two_at_myself(2, "False Peace", 0)]
#[case::four_at_myself(4, "False Peace", 0)]
#[case::two_ruse(2, "Empty City Ruse", 1)]
#[case::four_ruse(4, "Empty City Ruse", 2)]
fn r377_the_targets_next_turn_has_no_combat(
    #[case] seats: usize,
    #[case] spell: &str,
    #[case] target: usize,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [PresetPermanent::new(plains())])
        .hand(0, [card(spell).unwrap()]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    let at = driver.target_player(target);
    driver.cast_at(spell, &[at]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    // Two full rounds: the target's next turn and the one after it.
    let last = 3 + 2 * u32::try_from(seats).unwrap();
    driver.idle_until(last, Step::Upkeep).unwrap();
    let skipped_turn = if target == 0 {
        3 + u32::try_from(seats).unwrap()
    } else {
        3 + u32::try_from(target).unwrap()
    };
    for (number, active, combat) in turns(&driver) {
        if number == 3 || number >= last {
            continue;
        }
        assert_eq!(
            combat,
            number != skipped_turn,
            "turn {number} of seat {active}: combat {combat}"
        );
    }
    assert_eq!(
        driver.state().rule_effects().len(),
        0,
        "the rule ended with that turn"
    );
    replays(&scenario, &driver);
}

#[rstest]
#[case::two_seed_one(2, 1)]
#[case::two_seed_two(2, 2)]
#[case::three_seed_three(3, 3)]
#[case::four_seed_four(4, 4)]
fn r377_ral_skips_a_turn_for_each_head(#[case] seats: usize, #[case] seed: u64) {
    let mut ral = PresetPermanent::new(card("Ral Zarek, Guest Lecturer").unwrap());
    ral.counters = vec![(CounterKind::Loyalty, 7)];
    let scenario = Scenario::new(seats)
        .seed(seed)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ral]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    driver
        .begin_ability("Ral Zarek, Guest Lecturer", 3)
        .unwrap();
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let heads = driver
        .engine()
        .events()
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::CoinFlipped {
                    side: CoinSide::Heads,
                    ..
                }
            )
        })
        .count();
    let flips = driver
        .engine()
        .events()
        .iter()
        .filter(|e| matches!(e, Event::CoinFlipped { .. }))
        .count();
    assert_eq!(flips, 5, "five coins");
    // Enough turns for the other seats to go around six times.
    let rounds = 7;
    let last = 3 + u32::try_from(seats * rounds).unwrap();
    driver.idle_until(last, Step::Upkeep).unwrap();
    let all = turns(&driver);
    let end = all.iter().rposition(|(_, active, _)| *active == 0).unwrap();
    let later = &all[..=end];
    let foe_turns = later.iter().filter(|(_, active, _)| *active == 1).count();
    let my_turns = later
        .iter()
        .filter(|(number, active, _)| *active == 0 && *number > 3)
        .count();
    assert!(my_turns >= 5, "{my_turns} turns of mine");
    assert_eq!(foe_turns + heads, my_turns, "{heads} heads");
    assert!(driver.state().is_alive(pid(1)));
    replays(&scenario, &driver);
}
