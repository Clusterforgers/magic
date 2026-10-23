//! Goal: the monarch cards that need more than the designation work at tables of two, three and four seats: damage causes no loss of life to the monarch who has the Archon and to nobody else, a creature exiled until an opponent becomes the monarch returns when any opponent of its exiler becomes it and not when its exiler does, an ability that asks whether a player was the monarch as the turn began counts the designation held when the turn began, a creature that attacks the monarch gets the bonus of the attack trigger and one that attacks anyone else does not, a player's own commander passes the designation on when nobody has it and no other creature does, and the monarch at the end step draws the extra card of the Archivist.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::pid;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Commander;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{burn, free_creature, island, monarcher, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, monarch_fuzz_setup, play_game, ready};
use rstest::rstest;

use common::seeds::fuzz_games;

fn monarch(driver: &Driver) -> Option<usize> {
    driver.state().monarch().map(mtg_core::ids::PlayerId::index)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn power(driver: &Driver, name: &str) -> i32 {
    let id = driver.id_of(name).unwrap();
    characteristics(driver.state(), id).unwrap().power.unwrap()
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

/// Passes until the stack is empty, choosing `target` for the one target any trigger asks for.
fn settle(driver: &mut Driver, target: &str) -> Result<(), ScriptError> {
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseTargets(_) => {
                let chosen = driver.target_named(target)?;
                driver.choose_targets(&[chosen])?;
            }
            DecisionKind::OrderTriggers(options) => {
                let order: Vec<usize> = (0..options.triggers.len()).collect();
                driver.order_triggers(&order)?;
            }
            _ if driver.state().stack().is_empty() => break,
            _ => {
                driver.pass()?;
            }
        }
    }
    Ok(())
}

/// Each seat holds a Crown (an instant that makes its caster the monarch) and an Island and has a Wall (0/5) of its own,
/// and seat 0 holds a Mountain for a Bolt too.
fn table(seats: usize, extra_for_zero: Vec<PresetPermanent>) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        let mut permanents = vec![ready(island())];
        if seat == 0 {
            permanents.extend(extra_for_zero.iter().cloned());
        } else {
            permanents.push(ready(free_creature(&format!("Wall {seat}"), 0, 5)));
        }
        scenario = scenario
            .hand(seat, [monarcher(&format!("Crown {seat}"))])
            .battlefield(seat, permanents);
    }
    scenario
}

/// Seat `seat` casts its Crown the first time it has priority in the first main step of `turn`, and it resolves.
fn crown_in(driver: &mut Driver, turn: u32, seat: usize) -> Result<(), ScriptError> {
    driver.idle_until(turn, Step::Main1)?;
    for _ in 0..8 {
        if driver.decision()?.player == pid(seat) {
            break;
        }
        driver.pass()?;
    }
    driver.cast(&format!("Crown {seat}"))?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

/// Passes to the end step, attacking with nothing and blocking nothing.
fn to_end_step(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().turn().step() >= Step::End {
            break;
        }
        match driver.decision()?.kind {
            DecisionKind::DeclareAttackers(_) => {
                driver.attack(&[])?;
            }
            DecisionKind::DeclareBlockers(_) => {
                driver.block(&[])?;
            }
            _ => {
                driver.pass()?;
            }
        }
    }
    Ok(())
}

/// Passes to the end of the combat damage step, declaring no blockers.
fn through_damage(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().turn().step() > Step::CombatDamage {
            break;
        }
        if let DecisionKind::DeclareBlockers(_) = driver.decision()?.kind {
            driver.block(&[])?;
        } else {
            driver.pass()?;
        }
    }
    Ok(())
}

#[rstest]
#[case::nobody_is_the_monarch(None, 17)]
#[case::the_archons_controller_is_the_monarch(Some(1), 20)]
#[case::another_seat_is_the_monarch(Some(0), 17)]
fn the_archon_stops_the_loss_of_life_from_damage_only_while_its_controller_is_the_monarch(
    #[case] holder: Option<usize>,
    #[case] expected: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = table(seats, vec![ready(mountain())])
        .hand(0, [monarcher("Crown 0"), burn("Bolt", 3)]);
    scenario = scenario.battlefield(
        1,
        vec![
            ready(island()),
            ready(free_creature("Wall 1", 0, 5)),
            ready(card("Archon of Coronation").unwrap()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    if let Some(seat) = holder {
        crown_in(&mut driver, 1, seat)?;
    }
    driver.idle_until(1, Step::Main1)?;
    let target = driver.target_player(1);
    driver.cast_at("Bolt", &[target])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(life(&driver, 1), expected);
    Ok(())
}

#[rstest]
#[case::with_the_archon(true, 20)]
#[case::without_it(false, 18)]
fn combat_damage_to_the_monarch_with_the_archon_loses_no_life_and_still_passes_the_designation(
    #[case] archon: bool,
    #[case] expected: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = table(seats, vec![ready(free_creature("Raider", 2, 2))]);
    let mut seat_one = vec![ready(island()), ready(free_creature("Wall 1", 0, 5))];
    if archon {
        seat_one.push(ready(card("Archon of Coronation").unwrap()));
    }
    scenario = scenario.battlefield(1, seat_one);
    let mut driver = Driver::from_scenario(scenario)?;
    crown_in(&mut driver, 1, 1)?;
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Raider", 1)])?;
    through_damage(&mut driver)?;
    assert_eq!(life(&driver, 1), expected);
    assert_eq!(monarch(&driver), Some(0), "the attacker's controller took it");
    Ok(())
}

/// Seat 0 holds the Jailer with four Plains and an Island; every other seat holds a Crown and has a Wall.
fn jailer_table(seats: usize) -> Scenario {
    let mut extra: Vec<_> = (0..4).map(|_| ready(plains())).collect();
    extra.push(ready(island()));
    table(seats, extra).hand(0, [card("Palace Jailer").unwrap(), monarcher("Crown 0")])
}

/// Seat 0 casts the Jailer, which exiles the Wall of seat 1.
fn jail(seats: usize) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(jailer_table(seats))?;
    driver.idle_until(1, Step::Main1)?;
    driver.cast("Palace Jailer")?;
    settle(&mut driver, "Wall 1")?;
    Ok(driver)
}

#[rstest]
#[case::the_opponent_it_was_taken_from(2, 1, true)]
#[case::the_only_opponent(2, 1, true)]
#[case::another_opponent(3, 2, true)]
#[case::the_last_opponent(4, 3, true)]
#[case::its_exiler_taking_it_again(3, 0, false)]
fn the_creature_returns_when_an_opponent_of_the_jailer_becomes_the_monarch(
    #[case] seats: usize,
    #[case] taker: usize,
    #[case] returns: bool,
) -> Result<(), ScriptError> {
    let mut driver = jail(seats)?;
    assert_eq!(monarch(&driver), Some(0));
    assert!(driver.id_of("Wall 1").is_err(), "the Wall is in exile");
    crown_in(&mut driver, 1, taker)?;
    let back = driver.id_of("Wall 1").is_ok_and(|id| {
        driver.state().object(id).map(mtg_engine::object::GameObject::zone) == Some(Zone::Battlefield)
    });
    assert_eq!(back, returns);
    assert_eq!(monarch(&driver), Some(if returns { taker } else { 0 }));
    Ok(())
}

fn knights_table(seats: usize) -> Scenario {
    table(seats, vec![ready(card("Knights of the Black Rose").unwrap())])
}

#[rstest]
#[case::held_when_the_turn_began(true, false, 18, 22)]
#[case::nobody_held_it_when_the_turn_began(false, false, 20, 20)]
#[case::held_it_in_the_turn_it_was_taken_back(false, true, 20, 20)]
fn knights_of_the_black_rose_drain_when_their_controller_was_the_monarch_as_the_turn_began(
    #[case] held_before: bool,
    #[case] taken_this_turn: bool,
    #[case] foe_life: i32,
    #[case] my_life: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(knights_table(seats))?;
    if held_before {
        crown_in(&mut driver, 1, 0)?;
    }
    if taken_this_turn {
        // Seat 0 is not the monarch when turn 1 begins: it takes the designation in that turn and loses it again.
        crown_in(&mut driver, 1, 0)?;
        crown_in(&mut driver, 1, 1)?;
    } else {
        crown_in(&mut driver, 2, 1)?;
    }
    settle(&mut driver, "Wall 1")?;
    assert_eq!(monarch(&driver), Some(1));
    assert_eq!(life(&driver, 1), foe_life);
    assert_eq!(life(&driver, 0), my_life);
    Ok(())
}

fn mbaku_table(seats: usize) -> Scenario {
    table(
        seats,
        vec![
            ready(card("M'Baku, Jabari Chieftain").unwrap()),
            ready(free_creature("Raider", 2, 2)),
        ],
    )
}

#[rstest]
#[case::the_attacked_seat_is_the_monarch(Some(1), 1, 3)]
#[case::the_attacked_seat_is_not(None, 1, 2)]
#[case::another_seat_is_the_monarch(Some(2), 1, 2)]
fn m_baku_pumps_a_creature_that_attacks_the_monarch_and_no_other(
    #[case] holder: Option<usize>,
    #[case] attacked: usize,
    #[case] expected: i32,
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(mbaku_table(seats))?;
    if let Some(seat) = holder {
        crown_in(&mut driver, 1, seat)?;
    }
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Raider", attacked)])?;
    settle(&mut driver, "Raider")?;
    assert_eq!(power(&driver, "Raider"), expected);
    Ok(())
}

#[rstest]
fn m_baku_pumps_an_attacker_at_the_only_opponent_when_that_opponent_is_the_monarch(
    #[values(true, false)] monarch_is_foe: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(mbaku_table(2))?;
    crown_in(&mut driver, 1, usize::from(monarch_is_foe))?;
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Raider", 1)])?;
    settle(&mut driver, "Raider")?;
    assert_eq!(power(&driver, "Raider"), if monarch_is_foe { 3 } else { 2 });
    Ok(())
}

/// A Commander table where seat 0 has the commander "Cmd" (a free 2/2) and the Archivist, and seat 2 can take the designation.
fn archivist_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [free_creature("Cmd", 2, 2)]);
    for seat in 0..seats {
        let mut permanents = vec![ready(island())];
        if seat == 0 {
            permanents.push(ready(card("Archivist of Gondor").unwrap()));
            permanents.push(ready(free_creature("Raider", 2, 2)));
        } else {
            permanents.push(ready(free_creature(&format!("Wall {seat}"), 0, 5)));
        }
        scenario = scenario
            .hand(seat, [monarcher(&format!("Crown {seat}"))])
            .battlefield(seat, permanents);
    }
    scenario
}

#[rstest]
#[case::the_commander_with_no_monarch("Cmd", None, Some(0))]
#[case::another_creature_with_no_monarch("Raider", None, None)]
#[case::the_commander_while_another_seat_is_the_monarch("Cmd", Some(2), Some(2))]
fn the_archivist_makes_its_controller_the_monarch_when_their_commander_deals_combat_damage_and_nobody_is(
    #[case] attacker: &str,
    #[case] holder: Option<usize>,
    #[case] expected: Option<usize>,
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(archivist_table(seats))?;
    driver.idle_until(1, Step::Main1)?;
    if attacker == "Cmd" {
        driver.cast("Cmd")?;
        while !driver.state().stack().is_empty() {
            driver.pass()?;
        }
    }
    if let Some(seat) = holder {
        crown_in(&mut driver, 1, seat)?;
    }
    // Seat 0's next turn.
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::DeclareAttackers)?;
    driver.attack(&[(attacker, 1)])?;
    through_damage(&mut driver)?;
    settle(&mut driver, "Wall 1")?;
    assert_eq!(monarch(&driver), expected);
    Ok(())
}

#[rstest]
fn the_archivist_draws_for_whoever_is_the_monarch_at_their_end_step(
    #[values(1, 2)] holder: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(archivist_table(seats.max(holder + 1)))?;
    let turn = u32::try_from(holder).unwrap() + 1;
    crown_in(&mut driver, turn, holder)?;
    let before = hand(&driver, holder);
    to_end_step(&mut driver)?;
    settle(&mut driver, "Wall 1")?;
    // The monarch's own draw and the Archivist's.
    assert_eq!(hand(&driver, holder), before + 2);
    Ok(())
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_the_monarch_cards_finish_replay_and_pass_the_designation(#[case] seats: usize) {
    let (mut changes, mut returns) = (0, 0);
    for seed in 0..fuzz_games(60) {
        let setup = monarch_fuzz_setup(seats, seed).unwrap();
        let outcome = play_game(setup.clone(), seed)
            .unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        replay(&setup, &outcome.events).unwrap();
        for event in &outcome.events {
            match event {
                Event::MonarchChanged { .. } => changes += 1,
                Event::ZoneChanged {
                    from: Zone::Exile,
                    to: Zone::Battlefield,
                    ..
                } => returns += 1,
                _ => {}
            }
        }
    }
    assert!(changes > 0, "the designation changed hands");
    let _ = returns;
}

#[rstest]
#[case::two(2, 61)]
#[case::four(4, 62)]
fn a_game_with_the_monarch_cards_repeats_exactly_from_its_seed(
    #[case] seats: usize,
    #[case] seed: u64,
) {
    let play = || {
        let setup = monarch_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
}
