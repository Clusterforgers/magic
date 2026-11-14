//! Goal: aura swap (CR 702.65) exchanges the Aura with an Aura card from its controller's hand that may enchant what it
//! enchants: the card enters attached to that creature and the Aura returns to the hand; declining, or holding only Auras
//! that can't enchant it, moves nothing; at two to four seats, and each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, attached_to, ready};
use rstest::rstest;

use crate::common::casting::lands;

const WINGS: &str = "Arcanum Wings";

/// Seat 0 controls Grizzly Bears wearing Arcanum Wings and three Islands, and holds `hand`.
fn r6403_table(seats: usize, hand: &[&str]) -> Scenario {
    let mut board = vec![
        ready(card("Grizzly Bears").unwrap()),
        attached_to(ready(card(WINGS).unwrap()), 0, 0),
    ];
    board.extend(lands(3, island));
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .hand(0, hand.iter().map(|name| card(name).unwrap()).collect::<Vec<_>>())
}

fn r6403_settle(driver: &mut Driver, pick: Option<&str>) {
    for _ in 0..200 {
        let state = driver.state();
        if state.stack().is_empty() && state.resolving().is_none() && state.pending_triggers().is_empty() {
            return;
        }
        if let Ok(decision) = driver.decision()
            && let DecisionKind::ChooseCards(options) = &decision.kind
        {
            let chosen = pick
                .and_then(|name| {
                    options
                        .from
                        .iter()
                        .copied()
                        .find(|id| driver.state().object(*id).unwrap().card().name == name)
                })
                .into_iter()
                .collect();
            driver.answer(Answer::ChooseCards(CardsChoice(chosen))).unwrap();
            continue;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack does not empty");
}

fn r6403_swap(scenario: &Scenario, pick: Option<&str>) -> Driver {
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.begin_activation(WINGS).unwrap();
    if matches!(driver.decision().unwrap().kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    }
    r6403_settle(&mut driver, pick);
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    driver
}

fn r6403_in(driver: &Driver, zone: Zone, name: &str) -> bool {
    driver
        .state()
        .zones
        .get(zone)
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == name)
}

#[rstest]
fn r6403_the_aura_card_takes_its_place(#[values(2, 3, 4)] seats: usize) {
    let scenario = r6403_table(seats, &["Rancor"]);
    let driver = r6403_swap(&scenario, Some("Rancor"));
    let me = Zone::Hand(mtg_core::ids::PlayerId::from_index(0));
    assert!(r6403_in(&driver, me, WINGS));
    let rancor = driver.id_of("Rancor").unwrap();
    let bears = driver.id_of("Grizzly Bears").unwrap();
    let object = driver.state().object(rancor).unwrap();
    assert_eq!(object.zone(), Zone::Battlefield);
    assert_eq!(
        object.permanent().unwrap().attached_to().map(|host| host.id),
        Some(bears)
    );
}

#[rstest]
#[case::declined(&["Rancor"], None)]
#[case::only_a_land_aura(&["Wild Growth"], Some("Wild Growth"))]
fn r6403_nothing_moves_without_a_fitting_aura(
    #[values(2, 3, 4)] seats: usize,
    #[case] hand: &[&str],
    #[case] pick: Option<&str>,
) {
    let scenario = r6403_table(seats, hand);
    let driver = r6403_swap(&scenario, pick);
    assert!(r6403_in(&driver, Zone::Battlefield, WINGS));
    for name in hand {
        assert!(!r6403_in(&driver, Zone::Battlefield, name));
    }
}
