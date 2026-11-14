//! Goal: Mind Bomb lets each player discard up to three cards, then deals each player 3 minus the number of cards
//! that player discarded, at two to four seats with every count from none to three; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// How many cards the player in `seat` discards: seat 0 none, then one more per seat, at most three.
fn r6105_discards(seat: usize, offset: usize) -> usize {
    (seat + offset) % 4
}

#[rstest]
fn r6105_mind_bomb_deals_three_minus_the_discards(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] offset: usize,
) -> Result<(), ScriptError> {
    let filler = || {
        (0..3)
            .map(|_| card("Grizzly Bears").unwrap())
            .collect::<Vec<_>>()
    };
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(island())]);
    for seat in 0..seats {
        let mut hand = filler();
        if seat == 0 {
            hand.push(card("Mind Bomb").unwrap());
        }
        scenario = scenario.hand(seat, hand);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let before: Vec<i32> = (0..seats)
        .map(|seat| driver.state().player(pid(seat)).unwrap().life())
        .collect();
    driver.cast("Mind Bomb")?;
    for _ in 0..80 {
        if driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseCards(options) = &decision.kind {
            let seat = (0..seats)
                .find(|seat| pid(*seat) == decision.player)
                .unwrap();
            assert_eq!((options.min, options.max), (0, 3), "up to three cards");
            let take = r6105_discards(seat, offset);
            driver.answer(Answer::ChooseCards(CardsChoice(
                options.from[..take].to_vec(),
            )))?;
        } else {
            driver.answer_passively()?;
        }
    }
    assert!(driver.state().stack().is_empty());
    for seat in 0..seats {
        let take = r6105_discards(seat, offset);
        let life = driver.state().player(pid(seat)).unwrap().life();
        assert_eq!(
            before[seat] - life,
            3 - i32::try_from(take).unwrap(),
            "seat {seat}"
        );
        let left = driver.state().zone(Zone::Hand(pid(seat))).len();
        assert_eq!(left, 3 - take, "seat {seat} keeps the rest");
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
