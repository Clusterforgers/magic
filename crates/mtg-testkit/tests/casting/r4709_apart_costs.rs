//! Goal: a cost that takes cards "with different names" plays at two to four seats: Ormos, Archive Keeper's discard
//! offers the hand in groups by name, refuses two cards of one name, and draws once three names are discarded.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, giant, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls Ormos and three Islands and holds two Lightning Bolts, a Grizzly Bears and a Craw Wurm.
#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r4709_ormos_discards_three_names(#[case] seats: usize) -> Result<(), ScriptError> {
    let bolt = || card("Lightning Bolt").unwrap();
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Ormos, Archive Keeper").unwrap()),
                ready(island()),
                ready(island()),
                ready(island()),
            ],
        )
        .hand(0, [bolt(), bolt(), bear(), giant()])
        .library(0, (0..8).map(|_| island()).collect::<Vec<_>>())
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let hand: Vec<_> = driver
        .state()
        .zone(Zone::Hand(mtg_core::ids::PlayerId::from_index(0)))
        .to_vec();
    driver.begin_activation("Ormos, Archive Keeper")?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("the discard is asked: {:?}", driver.decision()?.kind)
    };
    assert_eq!(options.from.len(), 4);
    let mut groups = options.groups.clone();
    groups.sort_unstable();
    groups.dedup();
    assert_eq!(groups.len(), 3);
    assert!(
        driver
            .answer(Answer::ChooseCards(CardsChoice(vec![
                hand[0], hand[1], hand[2]
            ])))
            .is_err()
    );
    driver.answer(Answer::ChooseCards(CardsChoice(vec![
        hand[0], hand[2], hand[3],
    ])))?;
    while matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let me = mtg_core::ids::PlayerId::from_index(0);
    // The second Lightning Bolt stays and five cards are drawn.
    assert_eq!(driver.state().zone(Zone::Hand(me)).len(), 6);
    assert_eq!(driver.state().zone(Zone::Graveyard(me)).len(), 3);
    Ok(())
}
