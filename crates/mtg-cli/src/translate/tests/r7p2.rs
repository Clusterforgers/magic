//! Goal: the keyword actions Monstrosity, Adapt and Bolster on a `PutCounter` line map to the engine's own action with its number; each acts once as the rules say.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::{abilities, gaps};

fn creature(script: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Subject")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value(value).unwrap());
    }
    Arc::new(card)
}

fn settle(driver: &mut Driver) {
    for _ in 0..80 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle
            && matches!(
                driver.engine().decision().map(|d| &d.kind),
                Some(DecisionKind::Priority(_))
            )
        {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

fn activate(driver: &mut Driver) {
    driver.begin_activation("Subject").unwrap();
    driver.pay_tapping(&[], 0).unwrap();
    settle(driver);
}

fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

fn game(script: &str) -> Driver {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [ready(creature(script)), ready(free_creature("Big", 4, 4))],
        )
        .battlefield(1, [ready(free_creature("Theirs", 1, 1))]);
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
#[case::monstrosity("Monstrosity$ 3", 1, 3)]
#[case::monstrosity_twice_counts_once("Monstrosity$ 3", 2, 3)]
#[case::adapt("Adapt$ 2", 1, 2)]
#[case::adapt_twice_counts_once("Adapt$ 2", 2, 2)]
fn the_action_puts_counters_on_the_source_once(
    #[case] key: &str,
    #[case] times: usize,
    #[case] want: u32,
) {
    let script =
        format!("Name:Subject\nTypes:Creature Elf\nPT:2/2\nA:AB$ PutCounter | Cost$ 0 | {key}\n");
    assert_eq!(gaps(&script), Vec::<String>::new());
    let mut driver = game(&script);
    for _ in 0..times {
        activate(&mut driver);
    }
    assert_eq!(counters(&driver, "Subject"), want);
    assert_eq!(counters(&driver, "Big"), 0);
    assert_eq!(counters(&driver, "Theirs"), 0);
}

#[test]
fn bolster_goes_to_the_weakest_creature_you_control() {
    let script =
        "Name:Subject\nTypes:Creature Elf\nPT:2/2\nA:AB$ PutCounter | Cost$ 0 | Bolster$ 2\n";
    assert_eq!(gaps(script), Vec::<String>::new());
    let mut driver = game(script);
    activate(&mut driver);
    assert_eq!(counters(&driver, "Subject"), 2);
    assert_eq!(counters(&driver, "Big"), 0);
    assert_eq!(counters(&driver, "Theirs"), 0);
}

#[rstest]
#[case::other_key("Monstrosity$ 3 | Defined$ You")]
#[case::unknown_amount("Adapt$ Foo")]
fn a_shape_the_rule_does_not_hold_is_a_gap(#[case] key: &str) {
    let script = format!("Name:T\nTypes:Creature Elf\nA:AB$ PutCounter | Cost$ 0 | {key}\n");
    assert_eq!(gaps(&script).len(), 1);
}
