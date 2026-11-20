//! Goal: choices that fill one slot per card type (Release, Grime Gorger) take one permanent or card for each type, and a
//! permanent or card of several of the types fills only one of their slots: each player sacrifices one artifact, creature,
//! enchantment, land and planeswalker they control at tables of two to four seats (an artifact creature counts once),
//! and an attacking Grime Gorger exiles at most one card of each type from the defending player's graveyard and grows by
//! as many.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::state::GameState;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

/// How many permanents of the type the seat controls.
fn r110_count(state: &GameState, seat: usize, kind: CardType) -> usize {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|object| object.controller() == pid(seat))
                && mtg_engine::characteristics(state, **id)
                    .is_some_and(|chars| chars.types.contains(kind))
        })
        .count()
}

#[rstest]
fn r110_each_player_sacrifices_one_of_each_type(#[values(2, 3, 4)] seats: usize) {
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Main1);
    for seat in 0..seats {
        let mut own = vec![
            ready(card("Sol Ring").unwrap()),
            ready(card("Intangible Virtue").unwrap()),
            ready(free_creature(&format!("Bear {seat}"), 2, 2)),
            ready(free_creature(&format!("Cub {seat}"), 1, 1)),
            ready(plains()),
        ];
        if seat == 0 {
            own.extend((0..3).map(|_| ready(mountain())));
            own.extend((0..3).map(|_| ready(plains())));
        }
        scenario = scenario.battlefield(seat, own);
    }
    let scenario = scenario.hand(0, [card("Catch // Release").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let before: Vec<[usize; 4]> = (0..seats)
        .map(|seat| {
            [
                CardType::Artifact,
                CardType::Creature,
                CardType::Enchantment,
                CardType::Land,
            ]
            .map(|kind| r110_count(driver.state(), seat, kind))
        })
        .collect();
    driver.begin_cast("Catch // Release").unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    while !driver.state().stack().is_empty() {
        let decision = driver.decision().unwrap().clone();
        driver.answer(Driver::passive_answer(&decision)).unwrap();
    }
    for (seat, counts) in before.iter().enumerate() {
        let after = [
            CardType::Artifact,
            CardType::Creature,
            CardType::Enchantment,
            CardType::Land,
        ]
        .map(|kind| r110_count(driver.state(), seat, kind));
        assert_eq!(after, counts.map(|count| count - 1), "seat {seat}");
    }
}

#[rstest]
fn r110_an_artifact_creature_fills_one_slot(#[values(2, 4)] seats: usize) {
    // The only artifact is a creature as well: it goes as the artifact, and another creature goes as the creature.
    let mut own: Vec<_> = (0..3).map(|_| ready(mountain())).collect();
    own.extend((0..3).map(|_| ready(plains())));
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, own)
        .battlefield(
            seats - 1,
            [
                ready(card("Palladium Myr").unwrap()),
                ready(free_creature("Bear", 2, 2)),
            ],
        )
        .hand(0, [card("Catch // Release").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Catch // Release").unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    while !driver.state().stack().is_empty() {
        let decision = driver.decision().unwrap().clone();
        driver.answer(Driver::passive_answer(&decision)).unwrap();
    }
    assert_eq!(r110_count(driver.state(), seats - 1, CardType::Creature), 0);
}

#[rstest]
fn r110_grime_gorger_exiles_one_card_of_each_type(#[values(2, 3, 4)] seats: usize) {
    let defender = seats - 1;
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(card("Grime Gorger").unwrap())])
        .graveyard(
            defender,
            [
                card("Palladium Myr").unwrap(),
                card("Sol Ring").unwrap(),
                card("Intangible Virtue").unwrap(),
                card("Liquid Fire").unwrap(),
                free_creature("Bear", 2, 2),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    let gorger = driver.id_of("Grime Gorger").unwrap();
    driver.attack(&[("Grime Gorger", defender)]).unwrap();
    // Each slot takes the first card it offers: an artifact, then a creature not taken as the artifact, then the
    // enchantment and the sorcery; no slot offers two cards.
    let mut asked = 0;
    while driver.state().turn().step() == Step::DeclareAttackers {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options) if !options.from.is_empty() => {
                assert_eq!(options.max, 1, "up to one card of each type");
                asked += 1;
                driver
                    .answer(Answer::ChooseCards(CardsChoice(vec![options.from[0]])))
                    .unwrap();
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision)).unwrap();
            }
        }
    }
    let left = driver.state().zone(Zone::Graveyard(pid(defender))).len();
    let counters = driver
        .state()
        .object(gorger)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    // Palladium Myr, Sol Ring, Intangible Virtue, Liquid Fire, Bear: two artifacts and two creatures share two slots.
    assert_eq!((left, counters), (1, 4));
    assert!(asked >= 4);
}
