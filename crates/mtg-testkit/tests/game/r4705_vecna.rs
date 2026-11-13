//! Goal: the equip ability of Hand of Vecna ("Equip—Pay 1 life for each card in your hand.") costs 1 life for each card
//! in its controller's hand as it is activated and attaches it, at tables of two to four seats; the log replays to the
//! same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::{Driver, Scenario, corpus, ready};
use rstest::rstest;

const HAND: &str = "Hand of Vecna";

fn table(seats: usize, cards: usize) -> Scenario {
    let own = vec![
        ready(free_creature("Bear", 2, 2)),
        ready(corpus::card(HAND).unwrap()),
    ];
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, own)
        .hand(0, (0..cards).map(|_| island()));
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| island()))
    })
}

#[rstest]
fn equipping_costs_a_life_for_each_card_in_hand(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 4)] cards: usize,
) {
    let mut driver = Driver::from_scenario(table(seats, cards)).unwrap();
    let start = driver
        .state()
        .player(mtg_core::ids::PlayerId(0))
        .unwrap()
        .life();
    let offered = driver.offered_abilities(HAND).unwrap();
    assert_eq!(
        offered.len(),
        1,
        "only the equip ability paid with life, with no mana"
    );
    driver.begin_ability(HAND, offered[0]).unwrap();
    let bear = driver.target_named("Bear").unwrap();
    driver.choose_targets(&[bear]).unwrap();
    for _ in 0..12 {
        if driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        driver.answer(Driver::passive_answer(&decision)).unwrap();
    }
    let life = driver
        .state()
        .player(mtg_core::ids::PlayerId(0))
        .unwrap()
        .life();
    assert_eq!(start - life, i32::try_from(cards).unwrap());
    let hand = driver.id_of(HAND).unwrap();
    let attached = driver
        .state()
        .object(hand)
        .unwrap()
        .permanent()
        .unwrap()
        .attached_to();
    assert_eq!(attached.map(|on| on.id), driver.id_of("Bear").ok());
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}
