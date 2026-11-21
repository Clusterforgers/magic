//! Goal: a delayed trigger on the next activation that spends at least an amount of mana (`Did Activated` under
//! `Condition::R113SpentFromTriggering`, CR 602.2b, 603.4, 707.10) at 2 to 4 seats: after Dynaheir's ability, an
//! ability activated for three mana is not copied and leaves the trigger waiting, and the next one activated for four
//! is copied once; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::plains;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::copying::{life, replays, settle};

const DYNAHEIR: &str = "Dynaheir, Invoker Adept";

/// An artifact with "{3}: Target player loses 1 life." and "{4}: Target player loses 1 life."
fn r114_rod() -> Arc<CardDefinition> {
    let ability = |mana: &str| {
        serde_json::from_str(&format!(
            r#"{{"Activated":{{"cost":"{mana}","targets":[{{"what":"Player"}}],"effects":[{{"LoseLife":{{"who":{{"Target":0}},"amount":1}}}}]}}}}"#
        ))
        .unwrap()
    };
    Arc::new(
        CardDefinition::new("Rod")
            .with_type(CardType::Artifact)
            .with_rules(ability("{3}"))
            .with_rules(ability("{4}")),
    )
}

fn table(seats: usize) -> Scenario {
    let mut own = vec![ready(card(DYNAHEIR).unwrap()), ready(r114_rod())];
    own.extend((0..7).map(|_| ready(plains())));
    Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, own)
}

/// Activates the Rod's ability with this index at seat 1 and resolves it.
fn r114_zap(driver: &mut Driver, ability: usize) {
    driver.begin_ability("Rod", ability).unwrap();
    let seat1 = driver.target_player(1);
    driver.choose_targets(&[seat1]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    settle(driver).unwrap();
}

#[rstest]
fn dynaheir_copies_the_next_ability_paid_with_four_mana(#[values(2, 3, 4)] seats: usize) {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    driver.begin_activation(DYNAHEIR).unwrap();
    settle(&mut driver).unwrap();
    r114_zap(&mut driver, 0);
    assert_eq!(life(&driver, 1), 19, "three mana: no copy");
    r114_zap(&mut driver, 1);
    assert_eq!(life(&driver, 1), 17, "four mana: the ability and its copy");
    replays(&driver, &setup);
}
