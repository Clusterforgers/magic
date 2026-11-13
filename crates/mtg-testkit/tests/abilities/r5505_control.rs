//! Goal: "The first player controls the second player during the second player's next turn, and the second player
//! controls the first player during the first player's next turn." (Cruel Entertainment) gives each target player
//! control of the other during the other's next turn, and not before, at tables of two to four, whoever casts it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::swamp;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

/// A corpus card for no mana.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut definition = (*card(name).unwrap()).clone();
    definition.mana_cost = Some(ManaCost::parse("{0}").unwrap());
    Arc::new(definition)
}

/// Passes until the turn of `player` reaches its first main phase.
fn until_main_of(driver: &mut Driver, player: PlayerId) {
    for _ in 0..2000 {
        let state = driver.state();
        if state.turn().active() == player
            && state.turn().step() == Step::Main1
            && driver.decision().is_ok()
        {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the turn of {player:?} does not come");
}

#[rstest]
#[case::two_seats_caster_first(2, 0, 1)]
#[case::two_seats_caster_second(2, 1, 0)]
#[case::three_seats_others(3, 1, 2)]
#[case::four_seats_others(4, 3, 1)]
#[case::four_seats_caster_and_next(4, 0, 1)]
fn each_target_controls_the_other_in_the_others_next_turn(
    #[case] seats: usize,
    #[case] first: usize,
    #[case] second: usize,
) {
    let mut scenario = Scenario::new(seats).hand(0, [free("Cruel Entertainment")]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..20).map(|_| swamp()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let (a, b) = (PlayerId::from_index(first), PlayerId::from_index(second));
    driver.begin_cast("Cruel Entertainment").unwrap();
    for _ in 0..20 {
        match &driver.decision().unwrap().kind {
            DecisionKind::ChooseTargets(options) => {
                let seat = if options.spec == 0 { first } else { second };
                assert!(
                    options.spec == 0 || !options.legal.contains(&driver.target_player(first)),
                    "another target player leaves the first out"
                );
                let target = driver.target_player(seat);
                driver.choose_targets(&[target]).unwrap();
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
    let controls = driver.state().player_controls().to_vec();
    assert_eq!(controls.len(), 2, "{controls:?}");
    assert!(controls.iter().any(|c| c.player == b && c.by == a));
    assert!(controls.iter().any(|c| c.player == a && c.by == b));
    // Nothing is under way in the turn the spell resolves in.
    for seat in 0..seats {
        let player = PlayerId::from_index(seat);
        assert_eq!(driver.state().player_controller(player), player);
    }
    // Turn order from seat 0: whichever target comes first is controlled by the other in their next turn.
    let order: Vec<PlayerId> = (1..=seats)
        .map(|step| PlayerId::from_index(step % seats))
        .collect();
    for player in order {
        if player != a && player != b {
            continue;
        }
        until_main_of(&mut driver, player);
        let other = if player == a { b } else { a };
        assert_eq!(driver.state().player_controller(player), other);
        assert_eq!(driver.state().player_controller(other), other);
    }
}
