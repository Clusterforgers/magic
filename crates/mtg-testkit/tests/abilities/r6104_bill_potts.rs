//! Goal: Bill Potts copies the first instant or sorcery spell its controller casts and the first ability they activate
//! each turn that targets only Bill Potts, once a turn in all; a spell or ability that also targets another creature,
//! one that targets another creature, and an opponent's spell are not copied; at two to four seats, and each log
//! replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::spell;

const BILL: &str = "Bill Potts";

/// "Ping deals 1 damage to target creature." with one target, or that and "tap target creature" with two.
fn r6104_ping(name: &str, places: usize) -> Arc<CardDefinition> {
    let targets = vec![r#"{"what":{"Object":[{"Type":"Creature"}]}}"#; places].join(",");
    let mut effects = r#"{"DealDamage":{"to":{"Target":0},"amount":1}}"#.to_owned();
    if places == 2 {
        effects.push_str(r#",{"Tap":{"what":{"Target":1}}}"#);
    }
    spell(
        name,
        CardType::Instant,
        &format!("[{targets}]"),
        &format!("[{effects}]"),
    )
}

/// An artifact named `name` with "<cost>: This artifact deals 1 damage to target creature."
fn r6104_rod(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name).with_type(CardType::Artifact).with_rules(
            serde_json::from_str(&format!(
                r#"{{"Activated":{{"cost":{cost},"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":1}}}}]}}}}"#
            ))
            .unwrap(),
        ),
    )
}

fn r6104_settle(driver: &mut Driver) {
    for _ in 0..200 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack does not empty");
}

/// The damage marked on the named creature.
fn r6104_damage(driver: &Driver, name: &str) -> u32 {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|o| o.card().name == name)
        .map(|o| o.permanent().unwrap().damage())
        .unwrap()
}

/// What seat `caster` does: cast a one-target Ping, a two-target Ping or activate the Rod, at Bill or the Bear.
#[derive(Clone, Copy, Debug)]
enum Act {
    PingBill,
    PingBear,
    PingBillAndBear,
    RodAtBill,
    RodAtBear,
    AltarAtBill,
}

fn r6104_act(driver: &mut Driver, act: Act, name: &str) {
    let bill = driver.target_named(BILL).unwrap();
    let bear = driver.target_named("Bear").unwrap();
    match act {
        Act::PingBill => driver.cast_at(name, &[bill]).unwrap(),
        Act::PingBear => driver.cast_at(name, &[bear]).unwrap(),
        Act::PingBillAndBear => {
            driver.begin_cast(name).unwrap();
            driver.choose_targets(&[bill]).unwrap();
            driver.choose_targets(&[bear]).unwrap();
            driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap()
        }
        Act::AltarAtBill => {
            driver.begin_activation("Altar").unwrap();
            for _ in 0..10 {
                match driver.decision().unwrap().kind.clone() {
                    DecisionKind::ChooseCards(_) => {
                        let fodder = driver.id_of("Fodder").unwrap();
                        driver
                            .answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
                                fodder,
                            ])))
                            .unwrap();
                    }
                    DecisionKind::ChooseTargets(_) => {
                        driver.choose_targets(&[bill]).unwrap();
                    }
                    DecisionKind::PayMana(_) => {
                        driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
                    }
                    _ => break,
                }
            }
            Vec::new()
        }
        Act::RodAtBill | Act::RodAtBear => {
            driver.begin_activation("Rod").unwrap();
            let at = if matches!(act, Act::RodAtBill) {
                bill
            } else {
                bear
            };
            let events = driver.choose_targets(&[at]).unwrap();
            if matches!(driver.decision().unwrap().kind, DecisionKind::PayMana(_)) {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap()
            } else {
                events
            }
        }
    };
    r6104_settle(driver);
}

/// Seat 0 controls Bill Potts, a 2/5 Bear and the Rod; `active` holds the spells.
fn r6104_table(seats: usize, active: usize) -> Scenario {
    let places = |name: &str| if name.contains("Two") { 2 } else { 1 };
    let hand: Vec<_> = ["Ping One", "Ping Two", "Ping Again"]
        .iter()
        .map(|name| r6104_ping(name, places(name)))
        .collect();
    Scenario::new(seats)
        .at(2, active, Step::Main1)
        .hand(active, hand)
        .battlefield(
            0,
            [
                ready(card(BILL).unwrap()),
                ready(creature("Bear", 1, &[], 2, 5)),
                ready(r6104_rod("Rod", r#""{0}""#)),
                ready(r6104_rod(
                    "Altar",
                    r#"[{"Sacrifice":{"filter":[{"Type":"Creature"},{"Not":{"Supertype":"Legendary"}}]}}]"#,
                )),
                ready(creature("Fodder", 0, &[], 1, 1)),
            ],
        )
}

#[rstest]
#[case::a_spell_at_bill(Act::PingBill, (2, 0))]
#[case::an_ability_at_bill(Act::RodAtBill, (2, 0))]
#[case::an_ability_paid_by_a_sacrifice_at_bill(Act::AltarAtBill, (2, 0))]
#[case::a_spell_at_bill_and_the_bear(Act::PingBillAndBear, (1, 0))]
#[case::a_spell_at_the_bear(Act::PingBear, (0, 1))]
#[case::an_ability_at_the_bear(Act::RodAtBear, (0, 1))]
fn r6104_the_first_spell_or_ability_targeting_only_bill_is_copied(
    #[values(2, 3, 4)] seats: usize,
    #[case] act: Act,
    #[case] dealt: (u32, u32),
) {
    let scenario = r6104_table(seats, 0);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let name = if matches!(act, Act::PingBillAndBear) {
        "Ping Two"
    } else {
        "Ping One"
    };
    r6104_act(&mut driver, act, name);
    assert_eq!(
        (r6104_damage(&driver, BILL), r6104_damage(&driver, "Bear")),
        dealt,
        "{act:?}"
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn r6104_bill_copies_only_once_each_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] ability_first: bool,
) {
    let scenario = r6104_table(seats, 0);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    if ability_first {
        r6104_act(&mut driver, Act::RodAtBill, "");
    } else {
        r6104_act(&mut driver, Act::PingBill, "Ping One");
    }
    r6104_act(&mut driver, Act::PingBill, "Ping Again");
    assert_eq!(
        r6104_damage(&driver, BILL),
        3,
        "copied once, then dealt once"
    );
}

#[rstest]
fn r6104_an_opponents_spell_is_not_copied(#[values(2, 3, 4)] seats: usize) {
    let scenario = r6104_table(seats, 1);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    r6104_act(&mut driver, Act::PingBill, "Ping One");
    assert_eq!(r6104_damage(&driver, BILL), 1);
}
