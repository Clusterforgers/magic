//! Goal: a Saga whose last chapter exiles it and returns it transformed (CR 712.14a) comes back at tables of two, three and four seats as a new object with its back face up (the creature with its own abilities and replacements, no lore counters, summoning sick, with the mana value of its front face) that is not sacrificed, enters under its controller's control, acts in later turns, stays a commander when it is one, is shown with its back face to every seat, and the corpus Sagas that return transformed play whole games that replay to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::ZoneCause;
use mtg_engine::format::Commander;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_testkit::cards::plains;
use mtg_testkit::saga_cards::{lore_spell, transforming_saga};
use mtg_testkit::{
    Driver, PlayOptions, Scenario, ScriptError, play_game_with, ready, transforming_saga_fuzz_setup,
    with_counters,
};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

const TALE: &str = "Tale";

/// A game with the Saga on seat 0's battlefield holding two lore counters, at the start of its upkeep.
fn before_the_last_chapter(seats: usize) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [with_counters(
                ready(transforming_saga(TALE)),
                CounterKind::lore(),
                2,
            )],
        )
        .at(1, 0, Step::Upkeep);
    Driver::from_scenario(scenario)
}

/// Answers until nothing is on the stack and nothing resolves; triggers go on the stack in the order listed. Returns
/// every event on the way.
fn settle(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut log = Vec::new();
    for _ in 0..200 {
        let ordering = matches!(driver.decision()?.kind, DecisionKind::OrderTriggers(_));
        let busy = !driver.state().stack().is_empty() || driver.state().resolving().is_some();
        if !busy && !ordering {
            return Ok(log);
        }
        let events = if let DecisionKind::OrderTriggers(options) = &driver.decision()?.kind {
            let all: Vec<usize> = (0..options.triggers.len()).collect();
            driver.order_triggers(&all)?
        } else {
            driver.answer_passively()?
        };
        log.extend(events);
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The Saga played through its last chapter: the driver, the id of the card, the counter of the object it was and what
/// happened since the chapter triggered.
fn returned(seats: usize) -> Result<(Driver, ObjectId, u32, Vec<Event>), ScriptError> {
    let mut driver = before_the_last_chapter(seats)?;
    let id = driver.id_of(TALE)?;
    let counter = driver.state().object(id).unwrap().counter();
    driver.pass_until(Step::Main1)?;
    let log = settle(&mut driver)?;
    Ok((driver, id, counter, log))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn lore(state: &GameState, id: ObjectId) -> u32 {
    state
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(&CounterKind::lore()))
}

#[rstest]
fn the_last_chapter_returns_the_saga_with_its_back_face_up(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, id, _, _) = returned(seats)?;
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.zone(), Zone::Battlefield);
    assert_eq!(object.controller(), pid(0));
    assert_eq!(object.face(), Some(1));
    let card = object.card();
    assert_eq!(card.name, "Hero of Tale");
    assert_eq!((card.power, card.toughness), (Some(3), Some(3)));
    assert!(card.is_type(mtg_core::card::CardType::Creature));
    assert!(!card.subtypes.iter().any(|subtype| subtype.0 == "Saga"));
    // The front face is a Saga card and the permanent is no Saga, so nothing sacrifices it.
    assert_eq!(object.whole_card().name, TALE);
    Ok(())
}

#[rstest]
fn the_permanent_is_a_new_object_with_no_counters_that_is_summoning_sick(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, id, before, _) = returned(seats)?;
    let object = driver.state().object(id).unwrap();
    // One zone change out of the battlefield and one back in (CR 400.7).
    assert_eq!(object.counter(), before + 2);
    assert_eq!(lore(driver.state(), id), 0);
    let permanent = object.permanent().unwrap();
    assert!(permanent.summoning_sick());
    assert_eq!(permanent.counters().count(), 0);
    Ok(())
}

#[rstest]
fn the_back_face_rules_apply_as_it_enters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let start = 20;
    let (driver, id, _, log) = returned(seats)?;
    // It enters tapped (a replacement of the back face) and gains 5 life when it enters (a trigger of the back face);
    // nothing of chapter III or the front face gains life.
    assert!(driver.state().object(id).unwrap().permanent().unwrap().tapped());
    assert_eq!(life(&driver, 0), start + 5);
    let gains: Vec<_> = log
        .iter()
        .filter(|event| matches!(event, Event::LifeGained { .. }))
        .collect();
    assert_eq!(gains.len(), 1, "{gains:?}");
    Ok(())
}

#[rstest]
fn the_log_shows_the_exile_the_return_tapped_and_the_turning_over(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (_, id, _, log) = returned(seats)?;
    let ours: Vec<&Event> = log
        .iter()
        .filter(|event| event.touched_objects().contains(&id))
        .filter(|event| {
            matches!(
                event,
                Event::ZoneChanged { .. } | Event::Transformed { .. } | Event::Entered { .. }
            )
        })
        .collect();
    assert!(
        matches!(
            ours.as_slice(),
            [
                Event::ZoneChanged {
                    from: Zone::Battlefield,
                    to: Zone::Exile,
                    cause: ZoneCause::Exiled,
                    ..
                },
                Event::ZoneChanged {
                    from: Zone::Exile,
                    to: Zone::Battlefield,
                    ..
                },
                Event::Entered {
                    tapped: true,
                    controller,
                    ..
                },
                Event::Transformed { face: Some(1), .. },
            ] if *controller == pid(0)
        ),
        "{ours:?}"
    );
    assert!(
        !log.iter().any(|event| matches!(
            event,
            Event::ZoneChanged {
                cause: ZoneCause::Sacrificed,
                ..
            }
        )),
        "the Saga is not sacrificed"
    );
    Ok(())
}

#[rstest]
fn a_transformed_permanent_has_the_mana_value_of_its_front_face(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, id, _, _) = returned(seats)?;
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.card().mana_value(), 0);
    assert_eq!(object.mana_value(), 2);
    Ok(())
}

#[rstest]
fn the_creature_attacks_in_its_controllers_next_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, id, _, _) = returned(seats)?;
    let next = 1 + u32::try_from(seats).unwrap();
    driver.idle_until(next, Step::DeclareAttackers)?;
    let permanent = driver.state().object(id).unwrap().permanent().unwrap();
    assert!(!permanent.tapped() && !permanent.summoning_sick());
    let events = driver.attack(&[("Hero of Tale", 1)])?;
    assert!(events.iter().any(|event| matches!(
        event,
        Event::AttackersDeclared { attackers, .. } if attackers.iter().any(|(object, _)| *object == id)
    )));
    Ok(())
}

#[rstest]
fn lore_counters_put_on_by_a_spell_reach_the_last_chapter_the_same_way(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand(0, [lore_spell("Surge", 2)])
        .battlefield(
            0,
            [
                ready(plains()),
                with_counters(ready(transforming_saga(TALE)), CounterKind::lore(), 1),
            ],
        )
        .at(1, 0, Step::Main2);
    let mut driver = Driver::from_scenario(scenario)?;
    let id = driver.id_of(TALE)?;
    let target = driver.target_named(TALE)?;
    driver.cast_at("Surge", &[target])?;
    settle(&mut driver)?;
    let object = driver.state().object(id).unwrap();
    assert_eq!((object.zone(), object.face()), (Zone::Battlefield, Some(1)));
    assert_eq!(lore(driver.state(), id), 0);
    Ok(())
}

#[rstest]
fn a_commander_saga_that_returns_transformed_is_still_the_commander(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [transforming_saga(TALE)])
        .hand(0, [lore_spell("Surge", 2)])
        .battlefield(0, [ready(plains()), ready(plains()), ready(plains())])
        .at(1, 0, Step::Main2);
    let mut driver = Driver::from_scenario(scenario)?;
    let id = driver.state().commanders().next().unwrap();
    driver.cast(TALE)?;
    for _ in 0..seats {
        driver.pass()?;
    }
    settle(&mut driver)?;
    assert_eq!(driver.state().object(id).unwrap().zone(), Zone::Battlefield);
    let target = driver.target_named(TALE)?;
    driver.cast_at("Surge", &[target])?;
    settle(&mut driver)?;
    // The exile and the return are one resolution, so the commander is never left in exile to be asked about.
    let object = driver.state().object(id).unwrap();
    assert_eq!((object.zone(), object.face()), (Zone::Battlefield, Some(1)));
    assert!(driver.state().is_commander(id));
    assert_eq!(driver.state().commanders().collect::<Vec<_>>(), [id]);
    Ok(())
}

#[test]
fn bot_games_with_sagas_that_return_transformed_return_them_and_the_creatures_act() {
    // Per game kind: returned transformed, attacked as the creature, kept on the battlefield, replayed.
    let seen = std::sync::Mutex::new([0u64; 4]);
    let games = fuzz_games(12);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let setup = transforming_saga_fuzz_setup(players, seed);
            let outcome = play_game_with(setup, seed, PlayOptions::default())
                .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = replay(
                &transforming_saga_fuzz_setup(players, seed),
                &outcome.events,
            )
            .unwrap_or_else(|error| panic!("seed {seed}: the log does not replay: {error}"));
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            let mut turned: Vec<ObjectId> = Vec::new();
            for window in outcome.events.windows(2) {
                match window {
                    [
                        Event::ZoneChanged {
                            object,
                            from: Zone::Exile,
                            to: Zone::Battlefield,
                            ..
                        },
                        Event::Transformed {
                            object: same,
                            face: Some(1),
                        },
                    ] if object == same => {
                        seen[0] += 1;
                        turned.push(*object);
                    }
                    _ => {}
                }
            }
            for event in &outcome.events {
                if let Event::AttackersDeclared { attackers, .. } = event
                    && attackers.iter().any(|(object, _)| turned.contains(object))
                {
                    seen[1] += 1;
                }
            }
            // The Saga that came back is a creature now and is not sacrificed as a Saga.
            for id in &turned {
                if let Some(object) = replayed.object(*id)
                    && object.zone() == Zone::Battlefield
                    && object.face() == Some(1)
                {
                    seen[2] += 1;
                    assert_eq!(lore(&replayed, *id), 0, "seed {seed}");
                }
            }
            seen[3] += 1;
        }
    });
    let [returned, attacked, kept, played] = seen.into_inner().unwrap();
    assert!(played > 0);
    assert!(returned > 0, "some Saga returned transformed");
    assert!(attacked > 0, "some transformed Saga attacked as a creature");
    assert!(kept > 0, "some transformed Saga was still on the battlefield");
}
