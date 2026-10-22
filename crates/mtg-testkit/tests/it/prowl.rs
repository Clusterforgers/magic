//! Goal: prowl is an alternative cost from the hand that is offered only on a turn in which a creature its caster controlled dealt combat damage to a player and shared a creature type with the card; the spell remembers it was cast that way for "if its prowl cost was paid"; the record ends with the turn; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::{choose_way, lands, pay, ways};
use crate::common::{name_of, on_battlefield, pid};
use mtg_cards::CardDefinition;
use mtg_engine::PresetPermanent;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, replay};
use mtg_testkit::cards::{forest, island, swamp};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

/// Seat 0 attacks seat 1 at the declare attackers step of turn 1 with `attacker` (a Faerie Rogue or a Bear), holds `hand` and
/// the lands, and every seat has a library.
fn table(seats: usize, attacker: &str, hand: &[&str], lands: Vec<PresetPermanent>) -> Scenario {
    let mut permanents = vec![ready(card(attacker))];
    permanents.extend(lands);
    (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::DeclareAttackers)
            .hand_named(0, hand.iter().copied())
            .battlefield(0, permanents),
        |scenario, seat| scenario.library(seat, (0..8).map(|_| forest())),
    )
}

/// Seat 0 attacks seat 1 with `attacker`, and the game goes on to the second main phase.
fn attack_and_go_on(driver: &mut Driver, attacker: &str) -> Result<(), ScriptError> {
    driver.attack(&[(attacker, 1)])?;
    driver.idle_until(1, Step::Main2)
}

/// Settles the stack: priority passes until it is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 casts `name` at seat 1 for the way that starts with `way`, when one is asked, and lets it resolve.
fn cast(driver: &mut Driver, name: &str, way: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, way)?;
    }
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        driver.choose_targets(&[driver.target_player(1)])?;
    }
    pay(driver)?;
    settle(driver)
}

/// Whether beginning the cast of Morsel Theft asks for the prowl cost among its ways (it is asked beside the mana cost when both
/// can be paid, and not at all when the mana cost is the only way).
fn offers_prowl(driver: &mut Driver) -> Result<bool, ScriptError> {
    driver.begin_cast("Morsel Theft")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        driver.choose_targets(&[driver.target_player(1)])?;
    }
    Ok(
        matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_))
            && ways(driver)?.iter().any(|way| way.starts_with("Prowl")),
    )
}

/// The log replays to the state the game is in.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn hand(driver: &Driver) -> usize {
    driver.state().zone(Zone::Hand(pid(0))).len()
}

fn life(state: &GameState, seat: usize) -> i32 {
    state.player(pid(seat)).unwrap().life()
}

#[rstest]
#[case::a_rogue_connected("Latchkey Faerie", true)]
#[case::a_creature_with_no_shared_type_connected("Grizzly Bears", false)]
fn prowl_is_offered_after_a_creature_that_shares_a_type_dealt_combat_damage(
    #[case] attacker: &str,
    #[case] offered: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        attacker,
        &["Morsel Theft"],
        lands(4, swamp)
            .into_iter()
            .chain(lands(1, island))
            .collect(),
    ))?;
    attack_and_go_on(&mut driver, attacker)?;
    assert_eq!(life(driver.state(), 1), 20 - if offered { 3 } else { 2 });
    assert_eq!(offers_prowl(&mut driver)?, offered);
    Ok(())
}

#[rstest]
fn nothing_is_offered_before_combat_damage_was_dealt(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table(seats, "Latchkey Faerie", &["Morsel Theft"], lands(4, swamp)).at(1, 0, Step::Main1),
    )?;
    assert!(!offers_prowl(&mut driver)?);
    Ok(())
}

#[rstest]
fn a_spell_cast_for_its_prowl_cost_draws_a_card_and_one_cast_for_its_mana_cost_does_not(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] prowled: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        "Latchkey Faerie",
        &["Morsel Theft"],
        lands(4, swamp),
    ))?;
    attack_and_go_on(&mut driver, "Latchkey Faerie")?;
    let before = life(driver.state(), 1);
    cast(
        &mut driver,
        "Morsel Theft",
        if prowled { "Prowl" } else { "Cast for" },
    )?;
    assert_eq!(life(driver.state(), 1), before - 3);
    assert_eq!(life(driver.state(), 0), 23);
    assert_eq!(
        hand(&driver),
        usize::from(prowled),
        "the card is drawn only with prowl"
    );
    assert_eq!(
        driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .filter(|id| name_of(driver.state(), **id) == "Morsel Theft")
            .count(),
        1
    );
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_creature_cast_for_its_prowl_cost_draws_when_it_enters(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] prowled: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        "Latchkey Faerie",
        &["Latchkey Faerie"],
        lands(4, island),
    ))?;
    attack_and_go_on(&mut driver, "Latchkey Faerie")?;
    cast(
        &mut driver,
        "Latchkey Faerie",
        if prowled { "Prowl" } else { "Cast for" },
    )?;
    assert_eq!(hand(&driver), usize::from(prowled));
    assert!(on_battlefield(driver.state(), "Latchkey Faerie").is_some());
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn the_prowl_cost_needs_its_mana(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    // {1}{B} cannot be paid with one Swamp: the card is not castable for it.
    let mut driver = Driver::from_scenario(table(
        seats,
        "Latchkey Faerie",
        &["Morsel Theft"],
        lands(1, swamp),
    ))?;
    attack_and_go_on(&mut driver, "Latchkey Faerie")?;
    assert!(driver.begin_cast("Morsel Theft").is_err());
    Ok(())
}

#[rstest]
fn the_record_ends_with_the_turn(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        "Latchkey Faerie",
        &["Morsel Theft"],
        lands(4, swamp),
    ))?;
    attack_and_go_on(&mut driver, "Latchkey Faerie")?;
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Main1)?;
    assert!(!offers_prowl(&mut driver)?);
    Ok(())
}
