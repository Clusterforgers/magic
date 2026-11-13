//! Goal: "convert" turns a More Than Meets the Eye card over as "transform" does, at tables of two to four: Flamewar
//! sacrifices another artifact for a +1/+1 counter and converts; its back face exiles as many cards face down as the
//! combat damage it deals to a player, puts an intel counter on each and converts back, and the front face puts every
//! exiled card its owner owns with an intel counter into their hand; Jetfire's back face converts and adapts 3, and its
//! front face gives the target player as much {C} as the +1/+1 counters removed, mana that only artifact spells,
//! abilities and other costs may use, then converts.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::spend::SpendOn;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const FLAMEWAR: &str = "Flamewar, Brash Veteran // Flamewar, Streetwise Operative";
const FLAMEWAR_FRONT: &str = "Flamewar, Brash Veteran";
const FLAMEWAR_BACK: &str = "Flamewar, Streetwise Operative";
const JETFIRE: &str = "Jetfire, Ingenious Scientist // Jetfire, Air Guardian";
const JETFIRE_FRONT: &str = "Jetfire, Ingenious Scientist";
const JETFIRE_BACK: &str = "Jetfire, Air Guardian";

fn corpus(name: &str) -> Arc<CardDefinition> {
    card(name).unwrap()
}

/// Answers every question (paying mana automatically) and passes until the stack is empty; returns the events.
fn drive(driver: &mut Driver) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return events,
            DecisionKind::PayMana(_) => {
                events.extend(driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap());
            }
            _ => events.extend(driver.answer_passively().unwrap()),
        }
    }
    panic!("the stack does not empty");
}

/// Whether the permanent shows its back face.
fn converted(driver: &Driver, id: ObjectId) -> bool {
    driver.state().object(id).unwrap().face() == Some(1)
}

fn plus_ones(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

/// Seat 0 holds Flamewar beside another artifact and lands; every library holds ten Swamps.
fn flamewar_table(seats: usize) -> Driver {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(corpus(FLAMEWAR)),
            ready(artifact_with("Scrap", vec![])),
            ready(mountain()),
            ready(swamp()),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| swamp()));
    }
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
fn flamewar_converts_hides_intel_and_brings_it_home(#[values(2, 3, 4)] seats: usize) {
    let mut driver = flamewar_table(seats);
    let flamewar = driver.id_of(FLAMEWAR_FRONT).unwrap();
    // Sacrifice another artifact: a +1/+1 counter, then the back face (a 2/1 Vehicle, a creature on its turn).
    driver.begin_ability(FLAMEWAR_FRONT, 1).unwrap();
    drive(&mut driver);
    assert!(
        driver.id_of("Scrap").is_err(),
        "the other artifact is sacrificed"
    );
    assert!(converted(&driver, flamewar));
    assert_eq!(plus_ones(&driver, flamewar), 1);
    // It attacks the last seat unblocked and deals 3 combat damage: three cards go face down with intel counters.
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[(FLAMEWAR_BACK, seats - 1)]).unwrap();
    driver.pass_until(Step::End).unwrap();
    let exiled: Vec<_> = driver.state().zone(Zone::Exile).to_vec();
    assert_eq!(exiled.len(), 3);
    for id in &exiled {
        let object = driver.state().object(*id).unwrap();
        assert_eq!(object.owner(), PlayerId(0));
        assert_eq!(
            object.card_counters_of(&CounterKind::Other("intel".into())),
            1
        );
    }
    assert!(!converted(&driver, flamewar), "it converts back");
    assert_eq!(driver.id_of(FLAMEWAR_FRONT).unwrap(), flamewar);
}

#[rstest]
fn flamewar_returns_only_its_owners_intel_cards(#[values(2, 3, 4)] seats: usize) {
    let mut driver = flamewar_table(seats);
    driver.begin_ability(FLAMEWAR_FRONT, 1).unwrap();
    drive(&mut driver);
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[(FLAMEWAR_BACK, 1)]).unwrap();
    driver.pass_until(Step::Main2).unwrap();
    assert_eq!(driver.state().zone(Zone::Exile).len(), 3);
    let before = driver.state().zone(Zone::Hand(PlayerId(0))).len();
    driver.begin_ability(FLAMEWAR_FRONT, 2).unwrap();
    drive(&mut driver);
    assert!(driver.state().zone(Zone::Exile).is_empty());
    assert_eq!(
        driver.state().zone(Zone::Hand(PlayerId(0))).len(),
        before + 3,
        "the discarded hand was empty and the three intel cards arrive"
    );
}

/// Seat 0 holds Jetfire with `counters` +1/+1 counters and three Islands.
fn jetfire_table(seats: usize, counters: u32) -> Driver {
    let jetfire = with_counters(
        ready(corpus(JETFIRE)),
        CounterKind::PlusOnePlusOne,
        counters,
    );
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        [jetfire, ready(island()), ready(island()), ready(island())],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| swamp()));
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Activates the printed ability `index` of the named permanent, removing `x` counters and targeting `target`'s player
/// when asked, then empties the stack; returns the events.
fn activate(driver: &mut Driver, name: &str, index: usize, x: u32, target: usize) -> Vec<Event> {
    let mut events = driver.begin_ability(name, index).unwrap();
    for _ in 0..50 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseNumber(_) => {
                events.extend(
                    driver
                        .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
                        .unwrap(),
                );
            }
            DecisionKind::ChooseTargets(_) => {
                let player = driver.target_player(target);
                events.extend(driver.choose_targets(&[player]).unwrap());
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return events,
            DecisionKind::PayMana(_) => {
                events.extend(driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap());
            }
            _ => events.extend(driver.answer_passively().unwrap()),
        }
    }
    panic!("the stack does not empty");
}

#[rstest]
fn jetfire_gives_the_removed_counters_as_colorless_mana_and_converts(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] target: usize,
    #[values(1, 2)] x: u32,
) {
    let mut driver = jetfire_table(seats, 2);
    let jetfire = driver.id_of(JETFIRE_FRONT).unwrap();
    let events = activate(&mut driver, JETFIRE_FRONT, 2, x, target);
    assert_eq!(plus_ones(&driver, jetfire), 2 - x);
    let added: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::ManaAdded {
                player, mana, only, ..
            } => Some((*player, *mana, only.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(added.len(), x as usize, "{added:?}");
    for (player, mana, only) in added {
        assert_eq!(player, PlayerId::from_index(target));
        assert_eq!(mana, ManaType::Colorless);
        assert_eq!(only.len(), 3);
        assert!(only.contains(&SpendOn::OtherCosts));
    }
    assert!(converted(&driver, jetfire), "Jetfire converts");
}

#[rstest]
fn jetfire_back_face_converts_then_adapts(#[values(2, 3, 4)] seats: usize, #[values(1, 2)] x: u32) {
    // The front face's ability (one or more of two counters removed) reaches the back face first.
    let mut driver = jetfire_table(seats, 2);
    let jetfire = driver.id_of(JETFIRE_FRONT).unwrap();
    activate(&mut driver, JETFIRE_FRONT, 2, x, 0);
    assert!(converted(&driver, jetfire));
    activate(&mut driver, JETFIRE_BACK, 2, 0, 0);
    assert!(!converted(&driver, jetfire), "it converts back");
    assert_eq!(
        plus_ones(&driver, jetfire),
        if x == 2 { 3 } else { 1 },
        "adapt 3 puts counters only on a permanent without +1/+1 counters"
    );
}
