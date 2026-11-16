//! Goal: Soul Immolation plays as printed at two to four seats: its X is at most the greatest toughness among its
//! caster's creatures, blighting X puts all X -1/-1 counters on one creature the caster picks (which needn't survive,
//! the blight rulings), and the spell deals X damage to each opponent and each creature they control, not to its
//! caster or their creatures; with no creature it can't be cast (no creature to blight).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

#[rstest]
#[case::all_of_the_wall(4)]
#[case::some(2)]
#[case::none(0)]
fn r6601_soul_immolation_blights_x_and_burns_the_opponents(
    #[values(2, 3, 4)] seats: usize,
    #[case] x: u32,
) {
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Soul Immolation").unwrap()])
        .battlefield(
            0,
            [
                ready(free_creature("Wall", 0, 4)),
                ready(free_creature("Bear", 2, 2)),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature(&format!("Foe{seat}"), 3, 3))]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let before: Vec<i32> = (0..seats)
        .map(|seat| driver.state().player(pid(seat)).unwrap().life())
        .collect();
    driver.begin_cast("Soul Immolation").unwrap();
    let DecisionKind::ChooseNumber(options) = &driver.decision().unwrap().kind else {
        panic!("X is asked: {}", driver.decision().unwrap().kind.name());
    };
    assert_eq!(options.max, Some(4), "the Wall's toughness");
    driver
        .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
        .unwrap();
    let wall = driver.id_of("Wall").unwrap();
    for _ in 0..30 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::ChooseCards(_) => {
                driver
                    .answer(Answer::ChooseCards(CardsChoice(vec![wall])))
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    let alive = |name: &str| driver.id_of(name).is_ok();
    assert_eq!(alive("Wall"), x < 4, "the Wall took X -1/-1 counters");
    if x < 4 {
        let held = driver.state().object(wall).unwrap().permanent().unwrap();
        assert_eq!(held.counters_of(&CounterKind::MinusOneMinusOne), x);
    }
    assert!(alive("Bear"), "the caster's creatures take no damage");
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), before[0]);
    for (seat, was) in before.iter().enumerate().skip(1) {
        let life = driver.state().player(pid(seat)).unwrap().life();
        assert_eq!(life, was - i32::try_from(x).unwrap());
        assert_eq!(alive(&format!("Foe{seat}")), x < 3);
    }
    assert_ne!(driver.state().zone(Zone::Graveyard(pid(0))).len(), 0);
}

#[rstest]
fn r6601_soul_immolation_needs_a_creature(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Soul Immolation").unwrap()])
        .battlefield(0, (0..5).map(|_| ready(mountain())));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert!(
        driver.begin_cast("Soul Immolation").is_err() || {
            let kind = &driver.decision().unwrap().kind;
            !matches!(kind, DecisionKind::ChooseNumber(options) if options.max.unwrap_or(0) > 0)
        }
    );
}
