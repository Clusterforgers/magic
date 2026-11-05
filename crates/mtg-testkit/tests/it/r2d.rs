//! Goal: an activation limit of two or three counts each use within a turn and frees at the next turn, and an ability others may activate is offered to those players only, costs no tap, and gives its effect to the activator.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Activators, Limit, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::refs::ZoneKind;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::gain_life;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// An artifact with a free ability: "You gain 1 life", with the limit and the players that may activate it.
fn gadget(by: Activators, limit: Option<Limit>, cost: Vec<CostItem>) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Gadget")
            .with_type(CardType::Artifact)
            .with_rules(Ability::Activated {
                cost: Cost(cost),
                timing: Timing::Instant,
                condition: None,
                limit,
                by,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: vec![gain_life(1)],
            }),
    )
}

fn offered(driver: &Driver) -> usize {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.abilities.len(),
        other => panic!("priority was expected, got {}", other.name()),
    }
}

fn use_once(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.begin_activation("Gadget")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::twice(2)]
#[case::three_times(3)]
fn a_limit_of_several_uses_stops_after_that_many_in_a_turn(
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let card = gadget(
        Activators::Controller,
        Some(Limit::TimesPerTurn(most)),
        vec![],
    );
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(card)]))?;
    for _ in 0..most {
        assert_eq!(offered(&driver), 1);
        use_once(&mut driver)?;
    }
    assert_eq!(offered(&driver), 0);
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        20 + most as i32
    );
    Ok(())
}

#[rstest]
#[case::anyone_controller(Activators::Anyone, 0, 1)]
#[case::anyone_other(Activators::Anyone, 1, 1)]
#[case::opponents_controller(Activators::Opponents, 0, 0)]
#[case::opponents_other(Activators::Opponents, 1, 1)]
#[case::controller_other(Activators::Controller, 1, 0)]
fn only_the_named_players_are_offered_the_ability(
    #[case] by: Activators,
    #[case] seat: usize,
    #[case] count: usize,
) -> Result<(), ScriptError> {
    let card = gadget(by, None, vec![]);
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(card)]).at(
        1,
        0,
        Step::Main1,
    ))?;
    if seat == 1 {
        driver.pass()?;
    }
    assert_eq!(driver.decision()?.player, pid(seat));
    assert_eq!(offered(&driver), count);
    Ok(())
}

#[test]
fn the_activator_gets_the_effect_of_an_ability_anyone_may_activate() -> Result<(), ScriptError> {
    let card = gadget(Activators::Anyone, None, vec![]);
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(card)]).at(
        1,
        0,
        Step::Main1,
    ))?;
    driver.pass()?;
    use_once(&mut driver)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 21);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20);
    Ok(())
}

#[test]
fn a_player_cannot_tap_a_permanent_they_do_not_control() -> Result<(), ScriptError> {
    let card = gadget(Activators::Anyone, None, vec![CostItem::Tap]);
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(card)]).at(
        1,
        0,
        Step::Main1,
    ))?;
    assert_eq!(offered(&driver), 1);
    driver.pass()?;
    assert_eq!(offered(&driver), 0);
    Ok(())
}
