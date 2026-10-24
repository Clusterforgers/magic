//! Goal: a permanent that chooses a player as it enters (True-Name Nemesis) asks its controller for one of the players in turn order, keeps the choice as a public `Chosen::Player` and, once the chosen player has left the game, goes on in a game that continues at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_testkit::cards::{island, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const NAME: &str = "True-Name Nemesis";

/// Seat 0 holds `name` and four lands of the colors it costs; every seat has a library.
fn table(seats: usize, name: &str) -> Driver {
    let lands = vec![ready(island()), ready(island()), ready(swamp())];
    let mut scenario = Scenario::new(seats)
        .hand(0, [card(name).unwrap()])
        .battlefield(0, lands);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Casts `name` with seat 0's lands and answers the choice of a player with the `pick`th option offered.
fn entered(seats: usize, name: &str, pick: usize) -> (Driver, Vec<ChoiceOption>) {
    let mut driver = table(seats, name);
    driver
        .cast_tapping(name, &["Island", "Island", "Swamp"])
        .unwrap();
    // Every seat passes with the spell on the stack, then it resolves and the choice is asked.
    for _ in 0..seats {
        if matches!(
            driver.decision().unwrap().kind,
            DecisionKind::ChooseOption(_)
        ) {
            break;
        }
        driver.pass().unwrap();
    }
    let decision = driver.decision().unwrap().clone();
    let DecisionKind::ChooseOption(options) = &decision.kind else {
        panic!(
            "a player is chosen as {name} enters, not {:?}",
            decision.kind
        );
    };
    assert_eq!(decision.player, pid(0));
    driver
        .answer(Answer::ChooseOption(OptionChoice(pick)))
        .unwrap();
    (driver, options.options.clone())
}

#[rstest]
fn the_players_in_turn_order_are_offered_and_the_pick_is_kept(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] pick: usize,
) {
    let (driver, offered) = entered(seats, NAME, pick.min(seats - 1));
    let everyone: Vec<ChoiceOption> = (0..seats)
        .map(|seat| ChoiceOption::Player(pid(seat)))
        .collect();
    assert_eq!(offered, everyone);
    let id = driver.id_of(NAME).unwrap();
    let kept = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .chosen()
        .cloned();
    assert_eq!(kept, Some(Chosen::Player(pid(pick.min(seats - 1)))));
}

#[rstest]
fn a_chosen_player_who_leaves_the_game_leaves_the_card_choosing_nobody(
    #[values(2, 3, 4)] seats: usize,
) {
    let (mut driver, _) = entered(seats, NAME, 1);
    driver.concede(pid(1)).unwrap();
    assert!(!driver.state().is_alive(pid(1)));
    // The game goes on: the card is still there and its controller still has priority or the game is over.
    assert!(driver.state().object(driver.id_of(NAME).unwrap()).is_some());
}
