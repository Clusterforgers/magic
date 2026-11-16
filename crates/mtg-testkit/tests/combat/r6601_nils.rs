//! Goal: Nils, Discipline Enforcer plays as printed at two to four seats: at its controller's end step it puts a +1/+1
//! counter on up to one target creature of each player (one target per player at most), and a creature with counters
//! costs {1} for each counter on it (every kind) to attack Nils's controller or their planeswalkers, nothing to attack
//! another player, and a creature without counters attacks freely (CR 508.1h).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const NILS: &str = "Nils, Discipline Enforcer";

#[rstest]
fn r6601_nils_counters_one_creature_of_each_player(#[values(2, 3, 4)] seats: usize) {
    let mut scenario = Scenario::new(seats).at(3, 0, Step::Main2).battlefield(
        0,
        [
            ready(card(NILS).unwrap()),
            ready(free_creature("Mine", 1, 1)),
        ],
    );
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(free_creature(&format!("First{seat}"), 1, 1)),
                ready(free_creature(&format!("Second{seat}"), 1, 1)),
            ],
        );
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let mut chosen = Vec::new();
    for _ in 0..40 {
        if driver.state().turn().number() > 3 {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::ChooseTargets(options) => {
                // Every creature is legal, but at most one per player is accepted.
                assert_eq!(
                    options.legal.len(),
                    2 + 2 * (seats - 1),
                    "Nils and Mine, two of each other seat"
                );
                let mut picks = vec![driver.target_named("Mine").unwrap()];
                for seat in 1..seats {
                    picks.push(driver.target_named(&format!("First{seat}")).unwrap());
                }
                if seats > 2 {
                    let mut two = picks.clone();
                    two.push(driver.target_named("Second1").unwrap());
                    assert!(
                        driver.choose_targets(&two).is_err(),
                        "two targets of one player"
                    );
                }
                chosen.clone_from(&picks);
                driver.choose_targets(&picks).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    assert_eq!(chosen.len(), seats);
    let counters = |name: &str| {
        let id = driver.id_of(name).unwrap();
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne)
    };
    assert_eq!(counters("Mine"), 1);
    for seat in 1..seats {
        assert_eq!(counters(&format!("First{seat}")), 1);
        assert_eq!(counters(&format!("Second{seat}")), 0);
    }
}

#[rstest]
#[case::two_counters(2, 0)]
#[case::three_of_two_kinds(1, 2)]
#[case::one_counter(1, 0)]
#[case::no_counter(0, 0)]
fn r6601_nils_taxes_attackers_by_their_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] plus: u32,
    #[case] other: u32,
) {
    let attacker = with_counters(
        with_counters(
            ready(free_creature("Raider", 1, 1)),
            CounterKind::PlusOnePlusOne,
            plus,
        ),
        CounterKind::Other("shield".into()),
        other,
    );
    let scenario = Scenario::new(seats)
        .at(3, 1, Step::DeclareAttackers)
        .battlefield(0, [ready(card(NILS).unwrap())])
        .battlefield(1, [attacker]);
    let driver = Driver::from_scenario(scenario).unwrap();
    let DecisionKind::DeclareAttackers(options) = &driver.decision().unwrap().kind else {
        panic!("seat 1 declares attackers");
    };
    let raider = driver.id_of("Raider").unwrap();
    let option = options
        .attackers
        .iter()
        .find(|o| o.object == raider)
        .unwrap();
    let tax_on = |target: AttackTarget| {
        option
            .tax
            .iter()
            .find(|(at, _)| *at == target)
            .map_or(0, |(_, cost)| cost.mana_value())
    };
    assert_eq!(tax_on(AttackTarget::Player(pid(0))), plus + other);
    for seat in 2..seats {
        assert_eq!(
            tax_on(AttackTarget::Player(pid(seat))),
            0,
            "seat {seat} is not Nils's"
        );
    }
}
