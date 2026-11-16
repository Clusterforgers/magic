//! Goal: Chaos Dragon plays as printed at two to four seats: at the beginning of combat on its controller's turn each
//! player rolls a d20, and the Dragon can't attack, this combat, the opponents who had the highest result (all of them
//! on a tie) or planeswalkers they control; it still attacks the others, and it may attack anyone when its controller
//! alone rolled highest (`Ref::R6601HighestRollers`, `Modification::R6601CantAttackPlayers`).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

#[rstest]
fn r6601_chaos_dragon_spares_the_highest_rollers(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 4, 5, 6)] seed: u64,
) {
    let scenario = Scenario::new(seats)
        .seed(seed)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(card("Chaos Dragon").unwrap())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let mut rolls = vec![None; seats];
    for _ in 0..40 {
        // With every opponent among the highest the Dragon has no one to attack, and no attack is asked.
        if driver.state().turn().step() == Step::Main2 {
            let rolled: Vec<u32> = rolls.iter().map(|roll| roll.unwrap()).collect();
            let highest = *rolled.iter().max().unwrap();
            assert!(
                rolled[1..].iter().all(|roll| *roll == highest),
                "rolls {rolled:?}"
            );
            return;
        }
        let decision = driver.decision().unwrap().clone();
        if let DecisionKind::DeclareAttackers(options) = &decision.kind {
            let dragon = driver.id_of("Chaos Dragon").unwrap();
            let option = options.attackers.iter().find(|o| o.object == dragon);
            let rolled: Vec<u32> = rolls.iter().map(|roll| roll.unwrap()).collect();
            let highest = *rolled.iter().max().unwrap();
            for seat in 1..seats {
                let open = option.is_some_and(|option| {
                    option.defenders.contains(&AttackTarget::Player(pid(seat)))
                });
                assert_eq!(
                    open,
                    rolled[seat] != highest,
                    "seat {seat}, rolls {rolled:?}"
                );
            }
            return;
        }
        for event in driver.answer_passively().unwrap() {
            if let Event::DieRolled {
                player,
                result,
                sides: 20,
                ..
            } = event
            {
                rolls[player.index()] = Some(result);
            }
        }
    }
    panic!("no attack was declared");
}
