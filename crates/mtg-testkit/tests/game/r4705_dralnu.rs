//! Goal: Dralnu, Lich Lord sacrifices that many permanents instead of being dealt damage (all it has when it has fewer),
//! and its ability gives an instant card in its controller's graveyard flashback for its mana cost until end of turn, so
//! the card is cast from the graveyard and exiled, at tables of two to four seats; the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, island, mountain};
use mtg_testkit::{Driver, Scenario, corpus, ready};
use rstest::rstest;

use crate::common::name_of;

const DRALNU: &str = "Dralnu, Lich Lord";

fn rule(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// "{T}: This artifact deals `amount` damage to target creature."
fn pinger(amount: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Pinger").with_type(CardType::Artifact).with_rules(rule(&format!(
            r#"{{"Activated":{{"cost":["Tap"],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]}}}}"#
        ))),
    )
}

/// A {R} instant: "You gain 2 life."
fn tonic() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Tonic")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[mtg_core::color::Color::Red]))
            .with_rules(rule(r#"{"Spell":{"effects":[{"GainLife":{"amount":2}}]}}"#)),
    )
}

fn settle(
    driver: &mut Driver,
    pick: &mut dyn FnMut(&Driver, &[mtg_core::ids::ObjectId]) -> Vec<mtg_core::ids::ObjectId>,
) {
    for _ in 0..30 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::ChooseCards(options) => {
                Answer::ChooseCards(CardsChoice(pick(driver, &options.from)))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
}

fn on_battlefield(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .count()
}

#[rstest]
fn damage_to_dralnu_is_that_many_sacrifices(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3, 6)] damage: u32,
) {
    let mut own = vec![ready(corpus::card(DRALNU).unwrap()), ready(pinger(damage))];
    own.extend((0..3).map(|_| ready(island())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, own);
    let scenario = (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..10).map(|_| island()))
    });
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Pinger").unwrap();
    let dralnu = driver.target_named(DRALNU).unwrap();
    driver.choose_targets(&[dralnu]).unwrap();
    // The Islands go first; Dralnu and the Pinger only when nothing else is left.
    let mut pick = |driver: &Driver, from: &[mtg_core::ids::ObjectId]| {
        let mut sorted = from.to_vec();
        sorted.sort_by_key(|id| name_of(driver.state(), *id) != "Island");
        let wanted = usize::try_from(damage).unwrap().min(sorted.len());
        sorted.truncate(wanted);
        sorted
    };
    settle(&mut driver, &mut pick);
    let sacrificed = usize::try_from(damage).unwrap().min(5);
    assert_eq!(on_battlefield(&driver, "Island"), 3 - sacrificed.min(3));
    if damage < 4 {
        let id = driver.id_of(DRALNU).unwrap();
        let damaged = driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .damage();
        assert_eq!(damaged, 0, "no damage is dealt to Dralnu");
    } else {
        assert!(driver.id_of(DRALNU).is_err(), "Dralnu was sacrificed");
    }
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
fn dralnu_gives_an_instant_flashback_for_its_mana_cost(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(corpus::card(DRALNU).unwrap()), ready(mountain())])
        .graveyard(0, [tonic()]);
    let scenario = (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..10).map(|_| island()))
    });
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert!(
        driver.begin_cast("Tonic").is_err(),
        "no flashback before the ability"
    );
    driver.begin_activation(DRALNU).unwrap();
    let state = driver.state();
    let card = state
        .zone(Zone::Graveyard(PlayerId(0)))
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == "Tonic")
        .unwrap();
    let tonic = mtg_engine::choose::Target::Object(state.object(card).unwrap().reference());
    driver.choose_targets(&[tonic]).unwrap();
    settle(&mut driver, &mut |_, from| from.to_vec());
    let life = driver.state().player(PlayerId(0)).unwrap().life();
    driver.begin_cast("Tonic").unwrap();
    driver.expect_decision(0, "pay-mana").unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    settle(&mut driver, &mut |_, from| from.to_vec());
    assert_eq!(driver.state().player(PlayerId(0)).unwrap().life(), life + 2);
    let state = driver.state();
    let exiled = state
        .zone(Zone::Exile)
        .iter()
        .any(|id| name_of(state, *id) == "Tonic");
    assert!(exiled, "a card cast with flashback is exiled");
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}
