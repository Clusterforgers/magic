//! Goal: Karn Liberated's restart (CR 727) puts every card back in its owner's library, starts the players over, runs the opening again with Karn's controller first, and once the opening is over puts the non-Aura permanent cards Karn exiled onto the battlefield under that player's control while an exiled Aura and a card exiled by something else stay out, at tables of two, three and four seats, and the game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_cards::value::CounterKind;
use mtg_engine::characteristics::controller_of;
use mtg_engine::events::Event;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::replay;
use mtg_engine::state::Stage;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

/// Seat 0 has Karn with 40 loyalty; seat 1 has Grizzly Bears enchanted by Pacifism and a creature exiled by nothing;
/// every seat has a library of ten creatures.
fn table(seats: usize) -> Scenario {
    let karn = with_counters(
        ready(card("Karn Liberated").unwrap()),
        CounterKind::Loyalty,
        40,
    );
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![karn])
        .battlefield(
            1,
            vec![
                ready(card("Grizzly Bears").unwrap()),
                attached_to(ready(card("Pacifism").unwrap()), 1, 0),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(
            seat,
            (0..10).map(|n| free_creature(&format!("Card {seat}.{n}"), 1, 1)),
        );
    }
    scenario
}

/// Activates Karn's loyalty ability `index` on target `target`, if any, and lets it resolve.
fn karn(driver: &mut Driver, index: usize, target: Option<&str>) -> Result<(), ScriptError> {
    driver.begin_ability("Karn Liberated", index)?;
    if let Some(name) = target {
        let target = driver.target_named(name)?;
        driver.choose_targets(&[target])?;
    }
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("Karn's ability".into()))
}

/// Answers passively until the restarted game's first turn has begun.
fn opened_again(driver: &mut Driver) -> Result<(), ScriptError> {
    let mut restarted = false;
    for _ in 0..200 {
        restarted = restarted
            || driver
                .engine()
                .events()
                .iter()
                .any(|event| matches!(event, Event::GameRestarted { .. }));
        if restarted
            && driver.state().stage() == Stage::Turns
            && driver.state().turn().number() == 1
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the new game's opening".into()))
}

#[rstest]
fn the_restart_keeps_what_karn_exiled(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    karn(&mut driver, 1, Some("Pacifism"))?;
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Main1)?;
    assert_eq!(
        names(driver.state(), Zone::Battlefield),
        vec!["Karn Liberated", "Grizzly Bears"]
    );
    let bears = driver.id_of("Grizzly Bears")?;
    karn(&mut driver, 1, Some("Grizzly Bears"))?;
    driver.idle_until(1 + 2 * u32::try_from(seats).unwrap(), Step::Main1)?;
    assert_eq!(
        names(driver.state(), Zone::Exile),
        vec!["Pacifism", "Grizzly Bears"]
    );
    let before = driver.engine().events().len();
    driver.begin_ability("Karn Liberated", 2)?;
    // The restart opens a new game: the opening asks its questions again.
    opened_again(&mut driver)?;
    let events = &driver.engine().events()[before..];
    assert!(
        events.iter().any(|event| matches!(
            event,
            Event::GameRestarted { by, kept } if *by == pid(0) && *kept == vec![bears]
        )),
        "{:?}",
        events
            .iter()
            .find(|event| matches!(event, Event::GameRestarted { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::MulliganPhaseBegan))
    );
    let state = driver.state();
    assert_eq!(state.turn().number(), 1);
    assert_eq!(state.turn().active(), pid(0));
    assert_eq!(state.first_player(), pid(0));
    // The kept card is on the battlefield under Karn's controller; nothing else is.
    assert_eq!(names(state, Zone::Battlefield), vec!["Grizzly Bears"]);
    assert_eq!(controller_of(state, bears), Some(pid(0)));
    assert!(names(state, Zone::Exile).is_empty());
    assert!(state.stack().is_empty());
    for seat in 0..seats {
        let player = state.player(pid(seat)).unwrap();
        assert_eq!(player.life(), 20);
        assert_eq!(player.counters().count(), 0);
        assert_eq!(state.zone(Zone::Hand(pid(seat))).len(), 7);
    }
    // Karn and the Aura are cards of their owners' new decks.
    let cards = |seat: usize| {
        let mut all = names(state, Zone::Hand(pid(seat)));
        all.extend(names(state, Zone::Library(pid(seat))));
        all
    };
    assert!(cards(0).contains(&"Karn Liberated".to_owned()));
    assert!(cards(1).contains(&"Pacifism".to_owned()));
    assert_eq!(cards(0).len(), 11);
    assert_eq!(cards(1).len(), 11);
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[test]
fn a_restart_keeps_nothing_karn_did_not_exile() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2).battlefield(
        1,
        vec![
            ready(card("Grizzly Bears").unwrap()),
            attached_to(ready(card("Pacifism").unwrap()), 1, 0),
            ready(free_creature("Bystander", 2, 2)),
        ],
    ))?;
    let bystander = driver.id_of("Bystander")?;
    driver.manual_by(
        pid(1),
        &ManualAction::Move {
            object: bystander,
            to: ManualZone::Exile,
        },
    )?;
    driver.begin_ability("Karn Liberated", 2)?;
    opened_again(&mut driver)?;
    let state = driver.state();
    assert!(
        names(state, Zone::Battlefield).is_empty(),
        "{:?} {:?} {}",
        names(state, Zone::Battlefield),
        state.stage(),
        state.turn().number()
    );
    assert!(names(state, Zone::Exile).is_empty());
    assert_eq!(state.zone(Zone::Hand(pid(1))).len(), 7);
    Ok(())
}
