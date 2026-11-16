//! Goal: the combat damage of a myriad token copy lowers the life of the player it attacks in the view of every seat and of a spectator, in a team game where the teammate of the attacker is no opponent and gets no copy.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_protocol::StateView;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 attacks seat 1 with a Patrol (myriad, 1 power) at a table where seats 0 and 3 play against seats 1 and 2, and
/// takes the copy for seat 2.
#[rstest]
#[case::teams(TeamMode::Teams)]
#[case::shared_turns(TeamMode::SharedTurns)]
fn the_view_of_every_seat_shows_the_damage_of_a_copy(#[case] mode: TeamMode) {
    let teams = match mode {
        TeamMode::SharedTurns => vec![0, 0, 1, 1],
        _ => vec![0, 1, 1, 0],
    };
    let (defender, other) = if mode == TeamMode::SharedTurns {
        (2, 3)
    } else {
        (1, 2)
    };
    let mut scenario = Scenario::new(4)
        .at(1, 0, Step::Main1)
        .house_rules(HouseRules {
            teams: Some(Teams { mode, seats: teams }),
            ..HouseRules::default()
        })
        .battlefield(0, [ready(card("Wyrm's Crossing Patrol").unwrap())]);
    for seat in 0..4 {
        scenario = scenario.library(seat, (0..8).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(1, Step::DeclareAttackers).unwrap();
    driver
        .attack(&[("Wyrm's Crossing Patrol", defender)])
        .unwrap();
    for _ in 0..20 {
        match &driver.decision().unwrap().kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
            }
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass().unwrap();
            }
            _ => break,
        }
    }
    driver.idle_until(1, Step::Main2).unwrap();
    let views = (0..4)
        .map(|seat| StateView::for_player(driver.state(), PlayerId::from_index(seat)))
        .chain([StateView::for_spectator(driver.state())]);
    for view in views {
        let lives: Vec<i32> = view
            .table
            .players
            .iter()
            .map(|player| player.life)
            .collect();
        let mut expected = vec![20; 4];
        expected[defender] = 19;
        expected[other] = 19;
        assert_eq!(lives, expected, "every seat's life as the view shows it");
    }
}
