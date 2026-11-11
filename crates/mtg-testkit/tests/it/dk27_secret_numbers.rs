//! Goal: Wheel of Misfortune at 2, 3 and 4 seats asks each player for a number face down, the caster first and the
//! others in turn order, deals the highest number as damage to each player who chose it, and makes each player who did
//! not choose the lowest discard their hand and draw seven; ties at either end take every tied player, and the numbers
//! are logged face down.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::Event;
use mtg_engine::choose::{Choice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::pid;

const NOT_LOWEST: &str =
    r#"{"PlayersWhere":{"who":"EachPlayer","test":{"Not":{"Among":{"Bound":"lowest"}}}}}"#;

fn wheel() -> Arc<CardDefinition> {
    let spell = format!(
        r#"{{"Spell":{{"effects":[{{"ChooseNumbers":{{"secret":true,"as":"number","highest":"highest","lowest":"lowest"}}}},{{"DealDamage":{{"to":{{"Bound":"highest"}},"amount":{{"ChosenAs":"number"}}}}}},{{"Discard":{{"who":{NOT_LOWEST},"count":{{"Count":[{{"In":"Hand"}},{{"OwnedBy":"EachPlayer"}}]}}}}}},{{"Draw":{{"who":{NOT_LOWEST},"count":7}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Wheel")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str(&spell).unwrap()),
    )
}

/// Each seat holds two cards (seat 0 the Wheel besides) and a library of ten.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        let wheel = (seat == 0).then(wheel);
        scenario = scenario
            .hand(seat, wheel.into_iter().chain([forest(), forest()]))
            .library(seat, (0..10).map(|_| forest()));
    }
    scenario
}

#[rstest]
#[case::two_seats(&[3, 1], &[17, 20], &[7, 2])]
#[case::three_seats(&[3, 5, 1], &[20, 15, 20], &[7, 7, 2])]
#[case::a_tie_at_the_top(&[4, 4, 0], &[16, 16, 20], &[7, 7, 2])]
#[case::a_tie_at_the_bottom(&[2, 0, 0, 6], &[20, 20, 20, 14], &[7, 2, 2, 7])]
#[case::all_the_same(&[1, 1, 1], &[19, 19, 19], &[2, 2, 2])]
fn the_highest_is_dealt_and_all_but_the_lowest_draw_seven(
    #[case] numbers: &[u32],
    #[case] life: &[i32],
    #[case] hands: &[usize],
) -> Result<(), ScriptError> {
    let seats = numbers.len();
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.cast("Wheel")?;
    for _ in 1..seats {
        driver.pass()?;
    }
    driver.pass()?;
    for (seat, number) in numbers.iter().enumerate() {
        let decision = driver.decision()?.clone();
        assert_eq!(decision.player, pid(seat), "turn order from the caster");
        let DecisionKind::ChooseNumber(options) = &decision.kind else {
            panic!("a number is asked, not {:?}", decision.kind);
        };
        assert!(options.secret && options.max.is_none() && options.min == 0);
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(*number)))?;
    }
    let state = driver.state();
    for seat in 0..seats {
        let player = state.player(pid(seat)).unwrap();
        assert_eq!(player.life(), life[seat], "life of seat {seat}");
        assert_eq!(
            state.zone(Zone::Hand(pid(seat))).len(),
            hands[seat],
            "hand of seat {seat}"
        );
    }
    let secret = driver
        .engine()
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ChoiceMade {
                    choice: Choice::SecretNumber(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(secret, seats, "every number is logged face down");
    Ok(())
}
