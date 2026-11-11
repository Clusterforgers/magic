//! Goal: Pain's Reward at 2, 3 and 4 seats opens with the caster's bid, asks the others in turn order whether to top the
//! high bid and for how much, ends when the high bid comes back around to its bidder, and only the high bidder loses the
//! high bid in life and draws four cards; no bid is capped by the bidder's life.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::{NumberChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::pid;

const RULES: &str = r#"{"Spell":{"effects":[{"BidLife":{"as":"bid","winner":"bidder"}},{"LoseLife":{"who":{"Bound":"bidder"},"amount":{"ChosenAs":"bid"}}},{"Draw":{"who":{"Bound":"bidder"},"count":4}}]}}"#;

fn reward() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Reward")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str(RULES).unwrap()),
    )
}

/// A move of the bidding: a seat opens or tops with a number, or lets the high bid stand.
#[derive(Debug, Clone, Copy)]
enum Move {
    Bid(usize, u32),
    Pass(usize),
}

#[rstest]
#[case::nobody_tops(&[Move::Bid(0, 3), Move::Pass(1)], 0, 3)]
#[case::a_top_stands(&[Move::Bid(0, 2), Move::Bid(1, 5), Move::Pass(2), Move::Pass(0)], 1, 5)]
#[case::the_opener_tops_back(
    &[Move::Bid(0, 1), Move::Bid(1, 2), Move::Pass(2), Move::Bid(0, 15), Move::Pass(1), Move::Pass(2)],
    0,
    15
)]
#[case::around_four_seats(
    &[Move::Bid(0, 0), Move::Pass(1), Move::Pass(2), Move::Bid(3, 1), Move::Pass(0), Move::Pass(1), Move::Pass(2)],
    3,
    1
)]
fn the_high_bidder_pays_and_draws(
    #[case] moves: &[Move],
    #[case] winner: usize,
    #[case] bid: i32,
) -> Result<(), ScriptError> {
    let seats = moves
        .iter()
        .map(|m| match m {
            Move::Bid(seat, _) | Move::Pass(seat) => *seat,
        })
        .max()
        .unwrap()
        .max(1)
        + 1;
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        let hand: Vec<Arc<CardDefinition>> = if seat == 0 {
            vec![reward()]
        } else {
            Vec::new()
        };
        scenario = scenario
            .hand(seat, hand)
            .library(seat, (0..10).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Reward")?;
    for _ in 0..seats {
        driver.pass()?;
    }
    for (at, step) in moves.iter().enumerate() {
        let decision = driver.decision()?.clone();
        match (*step, &decision.kind) {
            // The opening bid is a number with no question before it.
            (Move::Bid(seat, number), DecisionKind::ChooseNumber(_)) if at == 0 => {
                assert_eq!(decision.player, pid(seat));
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(number)))?;
            }
            (Move::Bid(seat, number), DecisionKind::YesNo(_)) => {
                assert_eq!(decision.player, pid(seat), "move {at}");
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(number)))?;
            }
            (Move::Pass(seat), DecisionKind::YesNo(_)) => {
                assert_eq!(decision.player, pid(seat), "move {at}");
                driver.answer(Answer::YesNo(YesNoChoice(false)))?;
            }
            (step, kind) => panic!("move {at} {step:?} met {kind:?}"),
        }
    }
    let state = driver.state();
    for seat in 0..seats {
        let won = seat == winner;
        let life = state.player(pid(seat)).unwrap().life();
        assert_eq!(life, if won { 20 - bid } else { 20 }, "life of seat {seat}");
        let hand = state.zone(Zone::Hand(pid(seat))).len();
        assert_eq!(hand, if won { 4 } else { 0 }, "hand of seat {seat}");
    }
    assert!(state.stack().is_empty(), "the bidding is over");
    Ok(())
}

#[test]
fn a_top_must_be_higher_than_the_high_bid() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .hand(0, [reward()])
        .library(0, (0..10).map(|_| forest()))
        .library(1, (0..10).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Reward")?;
    driver.pass()?;
    driver.pass()?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(4)))?;
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("a higher bid is asked");
    };
    assert_eq!((options.min, options.max), (5, None));
    assert!(
        driver
            .answer(Answer::ChooseNumber(NumberChoice::Number(4)))
            .is_err()
    );
    Ok(())
}
