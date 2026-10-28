//! Goal: phase-out durations and phase triggers play whole at tables of two, three and four seats: a creature Oubliette phases out (with its Aura, a token or a commander alike) stays out through every untap step and phases in tapped, as the same object, the moment Oubliette leaves the battlefield; an Oubliette destroyed in response phases nothing out; of two Oubliettes only the one that leaves releases its creature; an Oubliette that phases out itself keeps its creature out; Teferi's Imp discards as it phases out and draws as it phases in; and every log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Commander;
use mtg_engine::object::GameObject;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, free_creature, plains, swamp};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

/// The objects named `name`, wherever they are, in id order.
fn w11b_all(driver: &Driver, name: &str) -> Vec<ObjectId> {
    let mut found: Vec<ObjectId> = driver
        .state()
        .objects()
        .filter(|object| object.card().name == name)
        .map(GameObject::id)
        .collect();
    found.sort();
    found
}

fn w11b_zone(driver: &Driver, id: ObjectId) -> Zone {
    driver.state().object(id).unwrap().zone()
}

fn w11b_target(driver: &Driver, id: ObjectId) -> Target {
    Target::Object(driver.state().object(id).unwrap().reference())
}

fn w11b_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Passes until the stack is empty, answering a target decision with `target`.
fn w11b_resolve(driver: &mut Driver, target: Option<ObjectId>) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            let chosen = w11b_target(driver, target.expect("a target"));
            driver.choose_targets(&[chosen])?;
            continue;
        }
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 casts an Oubliette with three Swamps; its trigger targets `creature`, and resolves unless `respond`.
fn w11b_oubliette(driver: &mut Driver, creature: ObjectId) -> Result<(), ScriptError> {
    driver.cast_tapping("Oubliette", &["Swamp", "Swamp", "Swamp"])?;
    w11b_resolve(driver, Some(creature))
}

/// Seat 0 destroys `what` with Beast Within, paid with three Forests.
fn w11b_beast_within(driver: &mut Driver, what: ObjectId) -> Result<(), ScriptError> {
    driver.begin_cast("Beast Within")?;
    let target = w11b_target(driver, what);
    driver.choose_targets(&[target])?;
    driver.pay_tapping(&["Forest", "Forest", "Forest"], 0)?;
    w11b_resolve(driver, Some(what))
}

/// What Oubliette holds in the tests: a creature with an Aura, a token, or seat 0's own commander.
#[derive(Debug, Clone, Copy)]
enum W11bHeld {
    Enchanted,
    Token,
    Commander,
}

fn w11b_table(seats: usize, held: W11bHeld) -> Scenario {
    let mut mine = lands(6, swamp);
    mine.extend(lands(6, forest));
    let scenario = Scenario::new(seats)
        .hand_named(0, ["Oubliette", "Beast Within", "Beast Within"])
        .battlefield(0, mine)
        .battlefield(
            1,
            [
                ready(free_creature("Wolf", 3, 3)),
                attached_to(ready(card("Rancor").unwrap()), 1, 0),
            ],
        );
    let scenario = (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..12).map(|_| forest()))
    });
    match held {
        W11bHeld::Commander => scenario
            .format(Arc::new(Commander))
            .commanders(0, [legend("Cmd", 0, &[], 2, 2)]),
        _ => scenario,
    }
}

#[rstest]
fn w11b_oubliette_holds_a_creature_until_it_leaves_and_it_phases_in_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[values(W11bHeld::Enchanted, W11bHeld::Token, W11bHeld::Commander)] held: W11bHeld,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(w11b_table(seats, held))?;
    let wolf = driver.id_of("Wolf")?;
    let creature = match held {
        W11bHeld::Enchanted => wolf,
        W11bHeld::Token => {
            // Beast Within on the Wolf gives seat 1 a Beast token.
            w11b_beast_within(&mut driver, wolf)?;
            driver.id_of("Beast")?
        }
        W11bHeld::Commander => {
            driver.cast("Cmd")?;
            w11b_resolve(&mut driver, None)?;
            driver.id_of("Cmd")?
        }
    };
    let counter = driver.state().object(creature).unwrap().counter();
    w11b_oubliette(&mut driver, creature)?;
    let oubliette = driver.id_of("Oubliette")?;
    assert_eq!(w11b_zone(&driver, creature), Zone::PhasedOut);
    let phasing = driver
        .state()
        .object(creature)
        .unwrap()
        .permanent()
        .unwrap()
        .phasing()
        .unwrap();
    assert_eq!(phasing.until.map(|by| by.id), Some(oubliette));
    assert!(phasing.tapped);
    let rancor = w11b_all(&driver, "Rancor")[0];
    if matches!(held, W11bHeld::Enchanted) {
        assert_eq!(
            w11b_zone(&driver, rancor),
            Zone::PhasedOut,
            "the Aura goes with it"
        );
    }
    // Two rounds of untap steps leave it out.
    let later = u32::try_from(2 * seats + 1).unwrap();
    driver.idle_until(later, Step::Main1)?;
    assert_eq!(w11b_zone(&driver, creature), Zone::PhasedOut);
    if matches!(held, W11bHeld::Commander) {
        assert!(driver.state().is_commander(creature));
    }
    w11b_beast_within(&mut driver, oubliette)?;
    assert_eq!(w11b_zone(&driver, creature), Zone::Battlefield);
    let back = driver.state().object(creature).unwrap();
    assert_eq!(back.counter(), counter, "the same object");
    assert!(back.permanent().unwrap().tapped(), "it phases in tapped");
    if matches!(held, W11bHeld::Enchanted) {
        assert_eq!(w11b_zone(&driver, rancor), Zone::Battlefield);
        assert_eq!(driver.state().host_of(rancor), Some(creature));
    }
    w11b_replays(&driver);
    Ok(())
}

#[rstest]
fn w11b_an_oubliette_destroyed_in_response_phases_nothing_out(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(w11b_table(seats, W11bHeld::Enchanted))?;
    let wolf = driver.id_of("Wolf")?;
    driver.cast_tapping("Oubliette", &["Swamp", "Swamp", "Swamp"])?;
    // The spell resolves; its trigger asks for a target, then seat 0 answers it with Beast Within.
    while !matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        driver.pass()?;
    }
    driver.choose_targets(&[w11b_target(&driver, wolf)])?;
    let oubliette = driver.id_of("Oubliette")?;
    w11b_beast_within(&mut driver, oubliette)?;
    assert_eq!(w11b_zone(&driver, wolf), Zone::Battlefield);
    assert!(driver.state().zone(Zone::PhasedOut).is_empty());
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::PermanentPhasedOut { .. }))
    );
    w11b_replays(&driver);
    Ok(())
}

#[rstest]
fn w11b_of_two_oubliettes_only_the_one_that_leaves_releases_its_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = lands(6, swamp);
    mine.extend(lands(3, forest));
    let scenario = Scenario::new(seats)
        .hand_named(0, ["Oubliette", "Oubliette", "Beast Within"])
        .battlefield(0, mine)
        .battlefield(
            1,
            [
                ready(free_creature("Wolf", 3, 3)),
                ready(free_creature("Ogre", 3, 3)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let (wolf, ogre) = (driver.id_of("Wolf")?, driver.id_of("Ogre")?);
    w11b_oubliette(&mut driver, wolf)?;
    w11b_oubliette(&mut driver, ogre)?;
    let holders: Vec<ObjectId> = w11b_all(&driver, "Oubliette")
        .into_iter()
        .filter(|id| w11b_zone(&driver, *id) == Zone::Battlefield)
        .collect();
    assert_eq!(holders.len(), 2);
    let of_ogre = holders
        .iter()
        .copied()
        .find(|id| {
            let ogre = driver.state().object(ogre).unwrap();
            ogre.permanent()
                .unwrap()
                .phasing()
                .unwrap()
                .until
                .map(|by| by.id)
                == Some(*id)
        })
        .unwrap();
    w11b_beast_within(&mut driver, of_ogre)?;
    assert_eq!(w11b_zone(&driver, ogre), Zone::Battlefield);
    assert_eq!(w11b_zone(&driver, wolf), Zone::PhasedOut);
    w11b_replays(&driver);
    Ok(())
}

#[rstest]
fn w11b_an_oubliette_that_phases_out_itself_keeps_its_creature_out(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = lands(3, swamp);
    mine.extend(lands(3, plains));
    let scenario = Scenario::new(seats)
        .hand_named(0, ["Oubliette", "Teferi's Protection"])
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("Wolf", 3, 3))]);
    let scenario = (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..12).map(|_| forest()))
    });
    let mut driver = Driver::from_scenario(scenario)?;
    let wolf = driver.id_of("Wolf")?;
    w11b_oubliette(&mut driver, wolf)?;
    let oubliette = driver.id_of("Oubliette")?;
    driver.cast_tapping("Teferi's Protection", &["Plains", "Plains", "Plains"])?;
    w11b_resolve(&mut driver, None)?;
    assert_eq!(w11b_zone(&driver, oubliette), Zone::PhasedOut);
    assert_eq!(
        w11b_zone(&driver, wolf),
        Zone::PhasedOut,
        "phasing out is no leaving"
    );
    // Seat 1's untap step brings back neither; seat 0's brings back Oubliette only.
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(w11b_zone(&driver, wolf), Zone::PhasedOut);
    driver.idle_until(u32::try_from(seats + 1).unwrap(), Step::Upkeep)?;
    assert_eq!(w11b_zone(&driver, oubliette), Zone::Battlefield);
    assert_eq!(w11b_zone(&driver, wolf), Zone::PhasedOut);
    w11b_replays(&driver);
    Ok(())
}

#[rstest]
fn w11b_teferis_imp_discards_as_it_phases_out_and_draws_as_it_phases_in(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("Teferi's Imp").unwrap())])
        .hand(0, (0..3).map(|_| forest()));
    let scenario = (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..12).map(|_| forest()))
    });
    let mut driver = Driver::from_scenario(scenario)?;
    let imp = driver.id_of("Teferi's Imp")?;
    let hand = |driver: &Driver| {
        driver
            .state()
            .zone(Zone::Hand(mtg_core::ids::PlayerId(0)))
            .len()
    };
    // Turn 1 has no draw for the first player; seat 0 plays nothing.
    let next = u32::try_from(seats + 1).unwrap();
    let before = hand(&driver);
    driver.idle_until(next, Step::Main1)?;
    assert_eq!(w11b_zone(&driver, imp), Zone::PhasedOut);
    // The phase-out trigger discarded a card in the upkeep, and the draw step drew one.
    assert_eq!(hand(&driver), before);
    let discards = |driver: &Driver| {
        driver
            .engine()
            .events()
            .iter()
            .filter(|event| matches!(event, Event::CardDiscarded { .. }))
            .count()
    };
    assert_eq!(discards(&driver), 1);
    driver.idle_until(2 * next - 1, Step::Main1)?;
    assert_eq!(w11b_zone(&driver, imp), Zone::Battlefield);
    // The phase-in trigger drew one and the draw step one more.
    assert_eq!(hand(&driver), before + 2);
    assert_eq!(discards(&driver), 1);
    w11b_replays(&driver);
    Ok(())
}
