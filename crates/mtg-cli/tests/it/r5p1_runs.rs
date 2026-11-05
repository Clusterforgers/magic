//! Goal: a step that remembers what it affected (the destroyed, the damaged) lets the lines after it read that object, and only the ones the step names.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::r3p3_runs::translated;

const DESTROY: &str = "Name:Snap\nManaCost:R\nTypes:Instant\nA:SP$ Destroy | ValidTgts$ Creature | RememberLKI$ True | SubAbility$ DBDraw | SpellDescription$ Destroy target creature. If a creature you controlled was destroyed this way, draw a card.\nSVar:DBDraw:DB$ Draw | ConditionDefined$ RememberedLKI | ConditionPresent$ Creature.YouCtrl | ConditionCompare$ EQ1 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n";
const DAMAGE: &str = "Name:Nick\nManaCost:R\nTypes:Instant\nA:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 1 | RememberDamaged$ True | SubAbility$ DBDraw | SpellDescription$ Nick deals 1 damage to target creature. If you control it, draw a card.\nSVar:DBDraw:DB$ Draw | ConditionDefined$ Remembered | ConditionPresent$ Creature.YouCtrl | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n";

/// Cards in seat 0's hand after it casts the spell at the bear of `owner` and everything resolved.
fn hand_after(script: &str, name: &str, owner: usize) -> usize {
    let card = translated(script, name, "Instant", "{R}");
    let mut mine = vec![ready(mountain())];
    let mut theirs = Vec::new();
    if owner == 0 {
        mine.push(ready(bear()));
    } else {
        theirs.push(ready(bear()));
    }
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, mine)
        .battlefield(1, theirs)
        .library(0, [forest(), forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named("Grizzly Bears").unwrap();
    driver.cast_at(name, &[target]).unwrap();
    for _ in 0..100 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    let state = driver.state();
    state.zone(Zone::Hand(PlayerId::from_index(0))).len()
}

#[rstest]
#[case::destroy_own("Snap", DESTROY, 0, 1)]
#[case::destroy_theirs("Snap", DESTROY, 1, 0)]
#[case::damage_own("Nick", DAMAGE, 0, 1)]
#[case::damage_theirs("Nick", DAMAGE, 1, 0)]
fn the_object_a_step_remembered_is_read_by_the_next(
    #[case] name: &str,
    #[case] script: &str,
    #[case] owner: usize,
    #[case] drawn: usize,
) {
    assert_eq!(hand_after(script, name, owner), drawn);
}
