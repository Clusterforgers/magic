//! Goal: "that creature can't attack during its controller's next turn" (`DuringItsControllersNextTurn`) changes nothing in the turn it begins and in the other seats' turns, holds the creature back in its controller's next turn and is gone after it; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::modify::Duration;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A 0/4 Wall with "Whenever ~ blocks a creature, that creature can't attack during its controller's next turn."
fn wall() -> Arc<CardDefinition> {
    let rule = r#"{"Triggered":{"when":{"BecomesBlocked":{"what":[],"by":["This"]}},"effects":[{"Continuous":{"affected":"Triggering","mods":["CantAttack"],"duration":"DuringItsControllersNextTurn"}}]}}"#;
    Arc::new(
        CardDefinition::new("Wall")
            .with_type(CardType::Creature)
            .with_power_toughness(0, 4)
            .with_rules(serde_json::from_str::<Ability>(rule).unwrap()),
    )
}

/// The names of the creatures the deciding player may attack with.
fn attackers(driver: &Driver) -> Vec<String> {
    let DecisionKind::DeclareAttackers(options) = &driver.decision().unwrap().kind else {
        panic!("not a declaration of attackers");
    };
    let mut names: Vec<String> = options
        .attackers
        .iter()
        .map(|a| driver.state().object(a.object).unwrap().card().name.clone())
        .collect();
    names.sort();
    names
}

/// Whether the restriction the Wall began applies now.
fn applies(driver: &Driver) -> bool {
    let state = driver.state();
    state.effects().iter().any(|effect| {
        effect.duration == Duration::DuringItsControllersNextTurn && effect.is_active(state)
    })
}

#[rstest]
fn the_blocked_creature_sits_out_its_controllers_next_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            vec![
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Runner", 1, 1)),
            ],
        )
        .battlefield(1, vec![ready(wall())]);
    for seat in 2..seats {
        scenario = scenario.battlefield(
            seat,
            vec![ready(free_creature(&format!("Ox {seat}"), 0, 1))],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Bear", 1)])?;
    driver.idle_until(1, Step::DeclareBlockers)?;
    driver.block(&[("Wall", "Bear")])?;
    driver.idle_until(1, Step::End)?;
    assert!(!applies(&driver), "not in the turn it began");
    let next = 1 + u32::try_from(seats).unwrap();
    for turn in 2..next {
        driver.idle_until(turn, Step::Main1)?;
        assert!(!applies(&driver), "not in turn {turn} of another seat");
    }
    driver.idle_until(next, Step::DeclareAttackers)?;
    assert!(applies(&driver));
    assert_eq!(attackers(&driver), ["Runner"]);
    driver.idle_until(next + u32::try_from(seats).unwrap(), Step::DeclareAttackers)?;
    assert!(!applies(&driver));
    assert_eq!(attackers(&driver), ["Bear", "Runner"]);
    Ok(())
}
