//! Goal: the keyword actions Monstrosity, Adapt and Bolster on a `PutCounter` line map to the engine's own action with its number; each acts once as the rules say.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, mountain};
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

fn spell(script: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Spell")
        .with_type(CardType::Sorcery)
        .with_mana_cost(mtg_testkit::cards::cost(0, &[mtg_core::color::Color::Red]));
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value(value).unwrap());
    }
    Arc::new(card)
}

fn exile_two(key: &str) -> String {
    format!(
        "Name:Spell\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Creature | TargetMin$ 2 | TargetMax$ 2 | {key}$ True\n"
    )
}

/// Seat 0 controls two bears and an ogre, seat 1 a bear and a cat; Spell is in hand.
fn table(script: &str) -> Driver {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [spell(script)])
        .battlefield(
            0,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Ogre", 3, 3)),
                ready(mountain()),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Cat", 1, 1)),
            ],
        );
    Driver::from_scenario(scenario).unwrap()
}

fn pick(driver: &Driver, seat: usize, name: &str) -> mtg_engine::choose::Target {
    let state = driver.state();
    let id = state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            state.object(*id).is_some_and(|object| {
                object.card().name == name && object.controller().index() == seat
            })
        })
        .unwrap();
    mtg_engine::choose::Target::Object(state.object(id).unwrap().reference())
}

/// The sets of two creatures (seat, name) that a rule allows (`true`) or refuses.
#[rstest]
#[case::same_controller_both_mine("TargetsWithSameController", [(0, "Bear"), (0, "Ogre")], true)]
#[case::same_controller_both_theirs("TargetsWithSameController", [(1, "Bear"), (1, "Cat")], true)]
#[case::same_controller_mixed("TargetsWithSameController", [(0, "Bear"), (1, "Cat")], false)]
#[case::different_controllers_mixed("TargetsWithDifferentControllers", [(0, "Ogre"), (1, "Cat")], true)]
#[case::different_controllers_same("TargetsWithDifferentControllers", [(0, "Bear"), (0, "Ogre")], false)]
#[case::different_names("TargetsWithDifferentNames", [(0, "Bear"), (1, "Cat")], true)]
#[case::same_name("TargetsWithDifferentNames", [(0, "Bear"), (1, "Bear")], false)]
fn a_set_of_targets_must_obey_the_rule(
    #[case] key: &str,
    #[case] set: [(usize, &str); 2],
    #[case] allowed: bool,
) {
    let script = exile_two(key);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let mut driver = table(&script);
    driver.begin_cast("Spell").unwrap();
    let targets: Vec<_> = set
        .iter()
        .map(|(seat, name)| pick(&driver, *seat, name))
        .collect();
    assert_eq!(driver.choose_targets(&targets).is_ok(), allowed);
}

#[rstest]
#[case::same_controller("TargetsWithSameController")]
#[case::different_controllers("TargetsWithDifferentControllers")]
#[case::different_names("TargetsWithDifferentNames")]
fn a_passive_chooser_always_finds_a_legal_set(#[case] key: &str) {
    let script = exile_two(key);
    let mut driver = table(&script);
    driver.begin_cast("Spell").unwrap();
    // The passive answer takes the first targets offered: they must obey the rule or the game would stall.
    assert!(driver.answer_passively().is_ok());
}

#[test]
fn a_rule_that_is_not_true_is_a_gap() {
    let script = exile_two("TargetsWithSameController").replace("$ True", "$ Maybe");
    assert_eq!(gaps(&script).len(), 1);
}

#[test]
fn no_set_rule_leaves_any_two_creatures_legal() {
    let script = "Name:Spell\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Creature | TargetMin$ 2 | TargetMax$ 2\n";
    let mut driver = table(script);
    driver.begin_cast("Spell").unwrap();
    let targets = [pick(&driver, 0, "Bear"), pick(&driver, 1, "Cat")];
    assert!(driver.choose_targets(&targets).is_ok());
}

#[test]
fn a_spell_whose_targets_cannot_differ_in_mana_value_cannot_be_cast() {
    // Every creature on the table costs 0, so no two differ.
    let script = exile_two("TargetsWithDifferentCMC");
    assert_eq!(gaps(&script), Vec::<String>::new());
    let mut driver = table(&script);
    assert!(driver.begin_cast("Spell").is_err());
}
