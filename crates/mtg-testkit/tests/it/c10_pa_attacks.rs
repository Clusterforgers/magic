//! Goal: "whenever a creature attacks you or a planeswalker you control" triggers once for each creature that attacks its controller or a planeswalker they control, and not for an attack at another player or at another player's planeswalker, at tables of two, three and four seats; "that creature's controller" is the attacking player, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, cost, walker};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free enchantment: "Whenever a creature attacks you or a planeswalker you control, that creature's controller loses
/// 1 life and you gain 1 life."
fn c10_pa_ravens() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"AttacksPlayerOrPlaneswalker":{"what":[{"Type":"Creature"}],"at":"You"}},"effects":[{"LoseLife":{"who":{"ControllerOf":"Triggering"},"amount":1}},{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Ravens")
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

fn c10_pa_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Where one of seat 0's Bears attacks.
#[derive(Clone, Copy, Debug)]
enum C10PaAt {
    Player(usize),
    Walker(usize),
}

/// Seat 1 controls the Ravens; seat 0 attacks with one Bear for each entry of `at`; every seat after 0 has a
/// planeswalker. The change of life of each seat is `lost` for seat 0 and `gained` for seat 1.
#[rstest]
#[case::the_controller(2, &[C10PaAt::Player(1)], 1)]
#[case::their_planeswalker(2, &[C10PaAt::Walker(1)], 1)]
#[case::both_trigger_twice(2, &[C10PaAt::Player(1), C10PaAt::Walker(1)], 2)]
#[case::another_player(3, &[C10PaAt::Player(2)], 0)]
#[case::another_players_planeswalker(3, &[C10PaAt::Walker(2)], 0)]
#[case::one_of_two(3, &[C10PaAt::Player(2), C10PaAt::Walker(1)], 1)]
#[case::four_seats(4, &[C10PaAt::Player(3), C10PaAt::Walker(2), C10PaAt::Player(1)], 1)]
fn c10_pa_an_attack_at_you_or_your_planeswalker_triggers(
    #[case] seats: usize,
    #[case] at: &[C10PaAt],
    #[case] triggers: i32,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut scenario = Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .battlefield(0, at.iter().map(|_| ready(bear())).collect::<Vec<_>>());
        for seat in 1..seats {
            let mut permanents = vec![ready(walker(&format!("Walker {seat}"), 2))];
            if seat == 1 {
                permanents.push(ready(c10_pa_ravens()));
            }
            scenario = scenario.battlefield(seat, permanents);
        }
        scenario
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let before: Vec<i32> = (0..seats).map(|seat| c10_pa_life(&driver, seat)).collect();
    driver.pass_until(Step::DeclareAttackers)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("seat 0 declares attackers");
    };
    let bears: Vec<ObjectId> = options.attackers.iter().map(|a| a.object).collect();
    let declared: Vec<(ObjectId, AttackTarget)> = bears
        .iter()
        .zip(at)
        .map(|(bear, at)| match at {
            C10PaAt::Player(seat) => (*bear, AttackTarget::Player(pid(*seat))),
            C10PaAt::Walker(seat) => (
                *bear,
                AttackTarget::Planeswalker(driver.id_of(&format!("Walker {seat}")).unwrap()),
            ),
        })
        .collect();
    assert_eq!(declared.len(), at.len());
    driver.answer(Answer::DeclareAttackers(declared))?;
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(
        c10_pa_life(&driver, 0) - before[0],
        -triggers,
        "the attacker"
    );
    assert_eq!(
        c10_pa_life(&driver, 1) - before[1],
        triggers,
        "the controller"
    );
    for (seat, was) in before.iter().enumerate().skip(2) {
        assert_eq!(c10_pa_life(&driver, seat), *was, "seat {seat}");
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
