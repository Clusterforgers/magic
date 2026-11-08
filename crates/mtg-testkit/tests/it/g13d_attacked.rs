//! Goal: "whenever an opponent attacks you and/or one or more planeswalkers you control" triggers once for a declaration that attacks its controller or a planeswalker they control, however many creatures do, and not for an attack only at another player or another player's planeswalker, at tables of two, three and four seats; "that player" is the attacking player, and the log replays to the live state.
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

/// A free enchantment: "Whenever an opponent attacks you and/or one or more planeswalkers you control, that player loses
/// 1 life and you gain 1 life."
fn g13d_threshold() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"},{"AttackingPlayerOrPlaneswalker":"You"}]}},"effects":[{"LoseLife":{"who":{"ControllerOf":"Triggering"},"amount":1}},{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Threshold")
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Where one of seat 0's Bears attacks.
#[derive(Clone, Copy, Debug)]
enum At {
    Player(usize),
    Walker(usize),
}

/// Seat 1 controls the Threshold; seat 0 attacks with one Bear for each entry of `at`; every seat after 0 has a
/// planeswalker. Seat 0 loses and seat 1 gains `triggers` life.
#[rstest]
#[case::the_controller(2, &[At::Player(1)], 1)]
#[case::their_planeswalker(2, &[At::Walker(1)], 1)]
#[case::both_trigger_once(2, &[At::Player(1), At::Walker(1)], 1)]
#[case::another_player(3, &[At::Player(2)], 0)]
#[case::another_players_planeswalker(3, &[At::Walker(2)], 0)]
#[case::one_of_two(3, &[At::Player(2), At::Walker(1)], 1)]
#[case::four_seats(4, &[At::Player(3), At::Walker(2), At::Player(1)], 1)]
fn g13d_an_attack_at_you_and_your_planeswalkers_triggers_once(
    #[case] seats: usize,
    #[case] at: &[At],
    #[case] triggers: i32,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut scenario = Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .battlefield(0, at.iter().map(|_| ready(bear())).collect::<Vec<_>>());
        for seat in 1..seats {
            let mut permanents = vec![ready(walker(&format!("Walker {seat}"), 2))];
            if seat == 1 {
                permanents.push(ready(g13d_threshold()));
            }
            scenario = scenario.battlefield(seat, permanents);
        }
        scenario
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let before: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
    driver.pass_until(Step::DeclareAttackers)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("seat 0 declares attackers");
    };
    let bears: Vec<ObjectId> = options.attackers.iter().map(|a| a.object).collect();
    let declared: Vec<(ObjectId, AttackTarget)> = bears
        .iter()
        .zip(at)
        .map(|(bear, at)| match at {
            At::Player(seat) => (*bear, AttackTarget::Player(pid(*seat))),
            At::Walker(seat) => (
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
    assert_eq!(life(&driver, 0) - before[0], -triggers, "the attacker");
    assert_eq!(life(&driver, 1) - before[1], triggers, "the controller");
    for (seat, was) in before.iter().enumerate().skip(2) {
        assert_eq!(life(&driver, seat), *was, "seat {seat}");
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
