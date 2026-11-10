//! Goal: a board of eighty creatures that tap for mana and a pool of twenty keeps every offer quick: the priority offer after each tap, the payment prompt and the automatic payment of a spell that convokes, improvises, delves or has a plain cost stay inside a bound that searching every subset of the creatures exceeds many times over, and the spell is still offered and paid.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Elves and druids that tap for mana (two kinds), and Sol Rings.
const ELVES: usize = 60;
const DRUIDS: usize = 20;
const RINGS: usize = 6;
/// The mana floated before the spell is cast.
const POOL: usize = 20;
/// Debug build on a loaded machine; the subset search took seconds for each offer of the turn.
const BOUND: Duration = Duration::from_secs(4);

#[rstest]
#[case::convoke("Venerated Loxodon")]
#[case::convoke_and_more("Feaster of Fools")]
#[case::improvise("Kappa Cannoneer")]
#[case::delve("Gurmag Angler")]
#[case::plain("Paradise Druid")]
fn a_board_of_mana_creatures_keeps_the_offer_quick(#[case] spell: &str) {
    let mut board = Vec::new();
    board.extend((0..ELVES).map(|_| ready(card("Llanowar Elves").unwrap())));
    board.extend((0..DRUIDS).map(|_| ready(card("Paradise Druid").unwrap())));
    board.extend((0..RINGS).map(|_| ready(card("Sol Ring").unwrap())));
    let hand = [
        spell,
        "Venerated Loxodon",
        "Feaster of Fools",
        "Lightning Bolt",
    ]
    .map(|name| card(name).unwrap());
    let scenario = Scenario::new(2)
        .battlefield(0, board)
        .hand(0, hand)
        .library(0, (0..10).map(|_| forest()))
        .library(1, (0..10).map(|_| forest()));
    let started = Instant::now();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let elves: Vec<_> = driver.state().zone(Zone::Battlefield)[..ELVES].to_vec();
    for _ in 0..POOL {
        let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
            panic!("priority was expected");
        };
        let source = options
            .mana_abilities
            .iter()
            .find(|option| elves.contains(&option.source.object))
            .unwrap()
            .source;
        driver
            .answer(Answer::Priority(PriorityAction::ActivateMana(source)))
            .unwrap();
    }
    driver.begin_cast(spell).unwrap();
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::PayMana(_)
    ));
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let took = started.elapsed();
    assert!(took < BOUND, "{spell}: {took:?}");
}
