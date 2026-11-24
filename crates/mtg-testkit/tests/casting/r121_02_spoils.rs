//! Goal: Spoils of War's X is the number of artifact and/or creature cards in the graveyard of an opponent the caster
//! picks as it is cast (CR 107.3c, 601.2b), at two to four seats: the X offered is one of the opponents' counts and no
//! other value (lands and instants in a graveyard do not count), a table where every opponent has the same count fixes X
//! without asking, the counters are distributed as X says, and each game replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::choose::{NumberChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{creature, instant, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 holds Spoils of War with nine Swamps and a Bear; opponent `seat` has `creatures` creature cards, a land and an
/// instant in their graveyard.
fn table(seats: usize, graveyards: &[usize]) -> Driver {
    let mut mine = vec![ready(creature("Bear", 2, &[Color::Green], 2, 2))];
    mine.extend(vec![ready(swamp()); 9]);
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .hand(0, [card("Spoils of War").unwrap()]);
    for seat in 1..seats {
        let creatures = graveyards.get(seat - 1).copied().unwrap_or(0);
        let mut cards: Vec<_> = (0..creatures)
            .map(|i| creature(&format!("Dead {seat}-{i}"), 1, &[Color::Red], 1, 1))
            .collect();
        cards.push(mountain());
        cards.push(instant("Spent Bolt", 1, &[Color::Red]));
        scenario = scenario.graveyard(seat, cards);
    }
    let scenario = (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![mountain(); 8])
    });
    Driver::from_scenario(scenario).unwrap()
}

/// Casts Spoils of War, answering X with `x` when asked and putting every counter on the Bear; the values offered for X
/// (none when X was fixed).
fn cast(driver: &mut Driver, x: u32) -> Option<Vec<u32>> {
    driver.begin_cast("Spoils of War").unwrap();
    let mut offered = None;
    for _ in 0..40 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseNumber(options) => {
                let max = options.max.unwrap();
                offered = Some(
                    (options.min..=max)
                        .filter(|value| !options.except.contains(value))
                        .collect(),
                );
                driver
                    .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
                    .unwrap();
            }
            DecisionKind::ChooseTargets(_) => {
                let bear = driver.target_named("Bear").unwrap();
                driver
                    .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![bear])))
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    offered
}

fn bear_counters(driver: &Driver) -> u32 {
    let bear = driver.id_of("Bear").unwrap();
    driver
        .state()
        .object(bear)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

#[rstest]
#[case::three_seats_two_counts(3, &[1, 3], 3, Some(vec![1, 3]))]
#[case::four_seats_three_counts(4, &[2, 0, 4], 2, Some(vec![0, 2, 4]))]
#[case::two_seats_one_count(2, &[2], 2, None)]
#[case::four_seats_one_shared_count(4, &[1, 1, 1], 1, None)]
fn spoils_of_war_x_is_an_opponents_count(
    #[case] seats: usize,
    #[case] graveyards: &[usize],
    #[case] x: u32,
    #[case] offered: Option<Vec<u32>>,
) {
    let mut driver = table(seats, graveyards);
    assert_eq!(cast(&mut driver, x), offered);
    assert_eq!(bear_counters(&driver), x);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
fn spoils_of_war_refuses_another_x(#[values(3, 4)] seats: usize) {
    let mut driver = table(seats, &[1, 3, 3]);
    driver.begin_cast("Spoils of War").unwrap();
    while !matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseNumber(_)
    ) {
        driver.answer_passively().unwrap();
    }
    assert!(
        driver
            .answer(Answer::ChooseNumber(NumberChoice::Number(2)))
            .is_err(),
        "2 is no opponent's count"
    );
}
