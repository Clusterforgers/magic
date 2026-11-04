//! Goal: audit faults of round 4 stay fixed: triggers on a cost that moves its source, mana abilities as activations, exiling the card a trigger names after it died, simultaneous damage removing one counter, and who orders a library.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{CostsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready, with_counters};

use crate::common::{names, pid};

fn def(name: &str, ty: CardType, pt: Option<(i32, i32)>, rules: &[&str]) -> Arc<CardDefinition> {
    let mut c = CardDefinition::new(name)
        .with_type(ty)
        .with_mana_cost(ManaCost::parse("{0}").unwrap());
    if let Some((p, t)) = pt {
        c = c.with_power_toughness(p, t);
    }
    for json in rules {
        c = c.with_rules(serde_json::from_str::<Ability>(json).unwrap());
    }
    Arc::new(c)
}

fn forest_lib(n: usize) -> Vec<Arc<CardDefinition>> {
    (0..n).map(|_| card("Forest").unwrap()).collect()
}

/// Answers every decision until the stack is empty and the active player has priority: costs none, mana automatic,
/// yes/no yes.
fn settle(dr: &mut Driver) {
    for _ in 0..300 {
        let dec = dr.decision().unwrap().clone();
        match &dec.kind {
            DecisionKind::Priority(_) if dr.state().stack().is_empty() => return,
            DecisionKind::ChooseCosts(_) => {
                dr.answer(Answer::ChooseCosts(CostsChoice::Costs(vec![])))
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                dr.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::YesNo(o) if !o.only_no => {
                dr.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
            }
            _ => {
                dr.answer_passively().unwrap();
            }
        }
    }
    panic!("did not settle: {}", dr.decision().unwrap().kind.name());
}

#[test]
fn exile_of_the_triggering_card_after_it_died_exiles_the_card() {
    let watcher = def(
        "Watcher",
        CardType::Artifact,
        None,
        &[
            r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Exile":{"what":"Triggering"}}]}}"#,
        ],
    );
    let sac = def(
        "Sac",
        CardType::Sorcery,
        None,
        &[r#"{"Spell":{"effects":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]}}"#],
    );
    let bear = def("Own Bear", CardType::Creature, Some((1, 1)), &[]);
    let sc = Scenario::new(2)
        .battlefield(0, [ready(watcher), ready(bear)])
        .hand(0, [sac])
        .library(0, forest_lib(9))
        .library(1, forest_lib(9));
    let mut dr = Driver::from_scenario(sc).unwrap();
    dr.begin_cast("Sac").unwrap();
    settle(&mut dr);
    assert_eq!(names(dr.state(), Zone::Exile), ["Own Bear"]);
    assert!(
        names(dr.state(), Zone::Graveyard(pid(0)))
            .iter()
            .all(|n| n != "Own Bear")
    );
}

fn hand_len(dr: &Driver, seat: usize) -> usize {
    names(dr.state(), Zone::Hand(pid(seat))).len()
}

/// A watcher that draws when its controller activates an ability of a creature.
fn activation_watcher() -> Arc<CardDefinition> {
    def(
        "Watcher",
        CardType::Artifact,
        None,
        &[
            r#"{"Triggered":{"when":{"Did":{"action":"Activated","who":"You","what":[{"Type":"Creature"},{"In":"Battlefield"}]}},"effects":[{"Draw":{}}]}}"#,
        ],
    )
}

#[test]
fn an_activation_whose_cost_sacrifices_its_source_still_triggers_for_a_creature() {
    let sacker = def(
        "Sacker",
        CardType::Creature,
        Some((1, 1)),
        &[r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"GainLife":{"amount":1}}]}}"#],
    );
    let sc = Scenario::new(2)
        .battlefield(0, [ready(activation_watcher()), ready(sacker)])
        .library(0, forest_lib(9))
        .library(1, forest_lib(9));
    let mut dr = Driver::from_scenario(sc).unwrap();
    dr.begin_activation("Sacker").unwrap();
    settle(&mut dr);
    assert!(names(dr.state(), Zone::Graveyard(pid(0))).contains(&"Sacker".to_string()));
    assert_eq!(hand_len(&dr, 0), 1);
}

const KNIGHT_REPLACEMENT: &str = r#"{"Replacement":{"when":{"Damage":{"to":{"Each":["This",{"HasCounter":"+1/+1"}]}}},"by":{"PreventRemovingCounter":"+1/+1"}}}"#;

fn knight_counters(dr: &Driver) -> u32 {
    let id = dr.id_of("Knight").unwrap();
    dr.state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

#[test]
fn damage_from_two_blockers_at_once_removes_one_counter() {
    let knight = def(
        "Knight",
        CardType::Creature,
        Some((0, 0)),
        &[KNIGHT_REPLACEMENT],
    );
    let bear = |n: &str| def(n, CardType::Creature, Some((1, 1)), &[]);
    let sc = Scenario::new(2)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [with_counters(ready(knight), CounterKind::PlusOnePlusOne, 3)],
        )
        .battlefield(1, [ready(bear("B1")), ready(bear("B2"))])
        .library(0, forest_lib(9))
        .library(1, forest_lib(9));
    let mut dr = Driver::from_scenario(sc).unwrap();
    dr.pass_until(Step::DeclareAttackers).unwrap();
    dr.attack(&[("Knight", 1)]).unwrap();
    for _ in 0..12 {
        if matches!(
            dr.decision().unwrap().kind,
            DecisionKind::DeclareBlockers(_)
        ) {
            break;
        }
        dr.pass().unwrap();
    }
    dr.block(&[("B1", "Knight"), ("B2", "Knight")]).unwrap();
    for _ in 0..60 {
        if dr.state().turn().step() == Step::End {
            break;
        }
        dr.answer_passively().unwrap();
    }
    assert_eq!(knight_counters(&dr), 2);
}

#[test]
fn damage_in_separate_events_removes_a_counter_each() {
    let knight = def(
        "Knight",
        CardType::Creature,
        Some((0, 0)),
        &[KNIGHT_REPLACEMENT],
    );
    let sc = Scenario::new(2)
        .battlefield(
            0,
            [
                with_counters(ready(knight), CounterKind::PlusOnePlusOne, 3),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand(
            0,
            [
                card("Lightning Bolt").unwrap(),
                card("Lightning Bolt").unwrap(),
            ],
        )
        .library(0, forest_lib(9))
        .library(1, forest_lib(9));
    let mut dr = Driver::from_scenario(sc).unwrap();
    for _ in 0..2 {
        dr.begin_cast("Lightning Bolt").unwrap();
        let t = dr.target_named("Knight").unwrap();
        dr.choose_targets(&[t]).unwrap();
        dr.pay_tapping(&["Mountain"], 0).unwrap();
        settle(&mut dr);
    }
    assert_eq!(knight_counters(&dr), 1);
}
