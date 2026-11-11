//! Goal: a target player a fact holds for (`Targetable::PlayerAmong` of a `PlayersWhere`, "target player who lost life this turn") offers only the players the fact holds for now, at two to four seats and whichever seat lost life: none before anyone lost life, so the spell is not offered, and the one who did once they have; the spell then resolves on them and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, mountain};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

const DRAIN: &str = r#"{"Spell":{"targets":[{"what":{"PlayerAmong":{"PlayersWhere":{"who":"EachPlayer","test":{"Compare":{"left":{"Tallied":{"who":"You","what":"LifeLost"}},"op":"Ge","right":1}}}}}}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":1}}]}}"#;

/// A sorcery for {0}: "Target player who lost life this turn loses 1 life."
fn drain() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(DRAIN).unwrap();
    Arc::new(
        CardDefinition::new("Drain")
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_core::mana::ManaCost::parse("{0}").unwrap())
            .with_rules(rule),
    )
}

#[rstest]
fn ep28_only_a_player_who_lost_life_is_a_target(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] hurt: usize,
) {
    if hurt >= seats {
        return;
    }
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [PresetPermanent::new(mountain())])
        .hand(0, [burn("Bolt", 3), drain()]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    assert!(
        driver.begin_cast("Drain").is_err(),
        "no player lost life yet, so there is no target"
    );
    let victim = driver.target_player(hurt);
    driver.cast_at("Bolt", &[victim]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    driver.begin_cast("Drain").unwrap();
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("a target is asked");
    };
    assert_eq!(options.legal, vec![victim], "only the player who lost life");
    driver.choose_targets(&[victim]).unwrap();
    let before = driver.state().player(pid(hurt)).unwrap().life();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(driver.state().player(pid(hurt)).unwrap().life(), before - 1);
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
