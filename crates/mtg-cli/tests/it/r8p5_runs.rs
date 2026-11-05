//! Goal: the round 8 piece 5 rules play in real games: a creature that would die goes to exile only when the line named it, support counters go on other creatures only, and an animated permanent leaves at the end step.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::r3p3_runs::{settle, translated};

fn count(driver: &Driver, zone: Zone, name: &str, _seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.card().name == name)
        .count()
}

fn grave(seat: usize) -> Zone {
    Zone::Graveyard(PlayerId::from_index(seat))
}

#[rstest]
#[case::lethal(4, 1, 0)]
#[case::survives(1, 0, 0)]
fn a_burned_creature_goes_to_exile_when_it_dies(
    #[case] damage: u32,
    #[case] exiled: usize,
    #[case] graveyard: usize,
) {
    let script = format!(
        "Name:Burn\nManaCost:R\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Creature | NumDmg${damage} | ReplaceDyingDefined$ Targeted | SpellDescription$ x\n"
    );
    let card = translated(&script, "Burn", "Sorcery", "{R}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [ready(bear())])
        .library(0, [mountain()])
        .library(1, [mountain()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Burn").unwrap();
    let target = driver.target_named("Grizzly Bears").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    settle(&mut driver);
    assert_eq!(count(&driver, Zone::Exile, "Grizzly Bears", 1), exiled);
    assert_eq!(count(&driver, grave(1), "Grizzly Bears", 1), graveyard);
    assert_eq!(
        count(&driver, Zone::Battlefield, "Grizzly Bears", 1),
        1 - exiled
    );
}

#[test]
fn a_creature_that_dies_later_in_the_turn_is_not_exiled() {
    let script = "Name:Burn\nManaCost:R\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 1 | ReplaceDyingDefined$ Targeted | SpellDescription$ x\n";
    let kill = "Name:Kill\nManaCost:R\nTypes:Instant\nA:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 5 | SpellDescription$ x\n";
    let scenario = Scenario::new(2)
        .hand(
            0,
            [
                translated(script, "Burn", "Sorcery", "{R}"),
                translated(kill, "Kill", "Instant", "{R}"),
            ],
        )
        .battlefield(0, [ready(mountain()), ready(mountain())])
        .battlefield(1, [ready(bear()), ready(free_creature("Other", 2, 2))])
        .library(0, [mountain()])
        .library(1, [mountain()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    for (spell, creature) in [("Burn", "Grizzly Bears"), ("Kill", "Other")] {
        driver.begin_cast(spell).unwrap();
        let target = driver.target_named(creature).unwrap();
        driver.choose_targets(&[target]).unwrap();
        driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
        settle(&mut driver);
    }
    assert_eq!(count(&driver, grave(1), "Other", 1), 1);
    assert_eq!(count(&driver, Zone::Exile, "Other", 1), 0);
}

#[rstest]
#[case::both_others(&["Grizzly Bears", "Grizzly Bears"], 2)]
#[case::one_other(&["Grizzly Bears"], 1)]
fn support_counters_go_on_other_creatures(#[case] picks: &[&str], #[case] expected: usize) {
    let script = "Name:Joraga Auxiliary Scout\nManaCost:G\nTypes:Creature Elf\nPT:1/1\nT:Mode$ ChangesZone | ValidCard$ Card.Self | Origin$ Any | Destination$ Battlefield | Execute$ P | TriggerDescription$ x\nSVar:P:DB$ PutCounter | Support$ 2\n";
    let card = translated(script, "Joraga Auxiliary Scout", "Creature — Elf", "{G}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(forest()), ready(bear()), ready(bear())])
        .library(0, [forest()])
        .library(1, [forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Joraga Auxiliary Scout").unwrap();
    for _ in 0..50 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseTargets(options) => {
                assert_eq!((options.min, options.max), (0, 2));
                let chosen: Vec<_> = options.legal.iter().copied().take(picks.len()).collect();
                driver.choose_targets(&chosen).unwrap();
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                let answer = Driver::passive_answer(&decision);
                driver.answer(answer).unwrap();
            }
        }
    }
    let state = driver.state();
    let (mut on_bears, mut on_aux) = (0, 0);
    for id in state.zone(Zone::Battlefield) {
        let object = state.object(*id).unwrap();
        let grown = object
            .permanent()
            .is_some_and(|p| p.counters_of(&CounterKind::PlusOnePlusOne) == 1);
        match object.card().name.as_str() {
            "Grizzly Bears" if grown => on_bears += 1,
            "Joraga Auxiliary Scout" if grown => on_aux += 1,
            _ => {}
        }
    }
    assert_eq!((on_bears, on_aux), (expected, 0));
}

#[test]
fn an_animated_artifact_is_sacrificed_at_the_end_step() {
    let script = "Name:Rig\nTypes:Artifact\nA:AB$ Animate | Cost$ R | Defined$ Self | Power$ 2 | Toughness$ 2 | Types$ Creature | Duration$ Permanent | AtEOT$ Sacrifice | SpellDescription$ x\n";
    let card = translated(script, "Rig", "Artifact", "");
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(card), ready(mountain())])
        .library(0, [mountain()])
        .library(1, [mountain()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Rig").unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    settle(&mut driver);
    assert_eq!(count(&driver, Zone::Battlefield, "Rig", 0), 1);
    let turn = driver.state().turn().number();
    driver.idle_until(turn + 1, Step::Upkeep).unwrap();
    assert_eq!(count(&driver, grave(0), "Rig", 0), 1);
}
