//! Goal: priority passes in turn order, any action restarts the round, and the stack resolves last-in-first-out only after everyone passes.

use crate::common;

use common::{names, pid, resolved};
use mtg_engine::Event;
use mtg_engine::decision::{DecisionKind, PriorityOptions};
use mtg_engine::stack::apnap_order;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, free_creature, instant};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn holder(driver: &Driver) -> Result<usize, ScriptError> {
    Ok(driver.decision()?.player.index())
}

fn options(driver: &Driver) -> Result<PriorityOptions, ScriptError> {
    match &driver.decision()?.kind {
        DecisionKind::Priority(o) => Ok(o.clone()),
        other => Err(ScriptError::UnexpectedDecision {
            wanted: "priority",
            got: other.name().to_owned(),
        }),
    }
}

#[rstest]
fn priority_circles_the_table_starting_with_the_active_player(
    #[values(2, 3, 4, 8)] players: usize,
    #[values(0, 1)] active: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(players).at(2, active, Step::Upkeep))?;
    let mut seen = Vec::new();
    for _ in 0..players {
        seen.push(holder(&driver)?);
        driver.pass()?;
    }
    let expected: Vec<usize> = (0..players).map(|i| (active + i) % players).collect();
    assert_eq!(seen, expected);
    driver.expect_step(Step::Draw)?;
    assert_eq!(
        holder(&driver)?,
        active,
        "the active player starts the next step"
    );
    Ok(())
}

#[rstest]
fn a_spell_waits_for_every_player_to_pass_in_succession(
    #[values(2, 3, 5)] players: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(players).hand(0, [free_creature("Ghoul", 1, 1)]))?;
    driver.cast("Ghoul")?;
    assert_eq!(driver.state().stack().len(), 1);
    assert_eq!(holder(&driver)?, 0, "the caster keeps priority");
    for passed in 1..players {
        driver.pass()?;
        assert_eq!(
            driver.state().stack().len(),
            1,
            "still on the stack after {passed} passes"
        );
    }
    driver.pass()?;
    assert_eq!(driver.state().stack().len(), 0);
    driver.expect_zone(Zone::Battlefield, &["Ghoul"])?;
    assert_eq!(
        holder(&driver)?,
        0,
        "the active player receives priority after resolution"
    );
    Ok(())
}

#[test]
fn the_stack_resolves_last_in_first_out() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(
        0,
        [
            instant("First", 0, &[]),
            instant("Second", 0, &[]),
            instant("Third", 0, &[]),
        ],
    ))?;
    for name in ["First", "Second", "Third"] {
        driver.cast(name)?;
    }
    assert_eq!(driver.state().stack().len(), 3);
    let mut events = Vec::new();
    for _ in 0..6 {
        events.extend(driver.pass()?);
    }
    assert_eq!(
        resolved(driver.state(), &events),
        ["Third", "Second", "First"]
    );
    driver.expect_zone(Zone::Graveyard(pid(0)), &["First", "Second", "Third"])?;
    Ok(())
}

#[test]
fn a_response_resolves_before_the_spell_it_answers() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .hand(0, [instant("Bolt", 0, &[])])
            .hand(1, [instant("Counter", 0, &[])]),
    )?;
    driver.cast("Bolt")?;
    driver.pass()?;
    assert_eq!(holder(&driver)?, 1);
    driver.cast("Counter")?;
    assert_eq!(
        holder(&driver)?,
        1,
        "the responder keeps priority after casting"
    );
    let mut events = Vec::new();
    // Casting reset the round, so all three players must pass again for each resolution.
    for _ in 0..6 {
        events.extend(driver.pass()?);
    }
    assert_eq!(resolved(driver.state(), &events), ["Counter", "Bolt"]);
    Ok(())
}

#[test]
fn an_action_restarts_the_round_of_passes() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::Upkeep)
            .battlefield(1, [ready(forest())]),
    )?;
    driver.pass()?;
    assert_eq!(holder(&driver)?, 1);
    driver.tap_for_mana("Forest")?;
    assert_eq!(
        holder(&driver)?,
        1,
        "activating a mana ability does not pass priority"
    );
    driver.pass()?;
    driver.pass()?;
    assert_eq!(
        holder(&driver)?,
        0,
        "the player who passed first must pass again"
    );
    driver.expect_step(Step::Upkeep)?;
    driver.pass()?;
    driver.expect_step(Step::Draw)
}

#[test]
fn mana_abilities_never_touch_the_stack() -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(forest()), ready(forest())]))?;
    let events = driver.tap_for_mana("Forest")?;
    assert_eq!(driver.state().stack().len(), 0);
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, Event::SpellCast { .. } | Event::SpellResolved { .. }))
    );
    assert_eq!(
        options(&driver)?.mana_abilities.len(),
        1,
        "the tapped Forest is no longer offered"
    );
    Ok(())
}

#[test]
fn only_instants_are_castable_off_timing() -> Result<(), ScriptError> {
    let hand = || [instant("Trick", 0, &[]), free_creature("Ghoul", 1, 1)];
    // Opponent's main phase: the non-active player holds priority after the active player passes.
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(1, hand()))?;
    driver.pass()?;
    assert_eq!(options(&driver)?.castable.len(), 1, "only the instant");
    // Own main phase with a spell on the stack: sorcery-speed spells must wait.
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(
        0,
        [
            instant("Trick", 0, &[]),
            instant("Echo", 0, &[]),
            free_creature("Ghoul", 1, 1),
        ],
    ))?;
    assert_eq!(options(&driver)?.castable.len(), 3);
    driver.cast("Trick")?;
    assert_eq!(
        options(&driver)?.castable.len(),
        1,
        "Ghoul needs an empty stack"
    );
    // Outside a main phase nothing but instants.
    let driver = Driver::from_scenario(Scenario::new(2).at(2, 0, Step::Upkeep).hand(0, hand()))?;
    assert_eq!(options(&driver)?.castable.len(), 1);
    Ok(())
}

#[test]
fn castable_requires_mana_that_exists() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [bear()])
            .battlefield(0, [ready(forest())]),
    )?;
    assert!(
        options(&driver)?.castable.is_empty(),
        "Grizzly Bears needs two mana"
    );
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [bear()])
            .battlefield(0, [ready(forest()), ready(forest())]),
    )?;
    assert_eq!(options(&driver)?.castable.len(), 1);
    Ok(())
}

#[rstest]
fn apnap_order_starts_with_the_active_player(
    #[values(2, 3, 4, 7)] players: usize,
    #[values(0, 1, 2)] active: usize,
) -> Result<(), ScriptError> {
    let active = active % players;
    let driver = Driver::from_scenario(Scenario::new(players).at(5, active, Step::Main1))?;
    let order: Vec<usize> = apnap_order(driver.state())
        .iter()
        .map(|p| p.index())
        .collect();
    let expected: Vec<usize> = (0..players).map(|i| (active + i) % players).collect();
    assert_eq!(order, expected);
    Ok(())
}

#[test]
fn creature_spells_become_summoning_sick_permanents() -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(2).hand(0, [free_creature("Ghoul", 1, 1)]))?;
    driver.cast("Ghoul")?;
    driver.pass()?;
    driver.pass()?;
    let state = driver.state();
    let id = state.zone(Zone::Battlefield)[0];
    let perm = state.object(id).and_then(|o| o.permanent());
    assert!(perm.is_some_and(|p| p.summoning_sick() && !p.tapped()));
    assert_eq!(names(state, Zone::Battlefield), ["Ghoul"]);
    Ok(())
}
