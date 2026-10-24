//! Goal: a prepare card is prepared when it enters or when its trigger says so, its controller casts a copy of its spell face from the battlefield at the timing of that spell and the permanent stops being prepared as the spell is cast, nobody else can, the copy is no card and leaves nothing behind, backing out of the cast keeps the permanent prepared, and a log with a prepared cast replays to the same state, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, apply_event};
use mtg_testkit::cards::{forest, mountain, plains};
use mtg_testkit::prepare_cards::{fox, hawk, owl};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, names, pid};

fn others(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 12]);
    }
    scenario
}

/// Every seat passes once: the top of the stack resolves.
fn resolve_top(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        driver.pass()?;
    }
    Ok(())
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

fn prepared(driver: &Driver, id: ObjectId) -> bool {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(|held| held.prepared())
}

/// Seat 0 holds a Soothing Owl and two Plains in the first main phase; the Owl is cast and has entered.
fn owl_entered(seats: usize) -> Result<(Driver, ObjectId), ScriptError> {
    let scenario = others(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![owl()])
        .battlefield(0, [ready(plains()), ready(plains())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Soothing Owl", &["Plains"])?;
    resolve_top(&mut driver, seats)?;
    let id = driver.id_of("Soothing Owl")?;
    Ok((driver, id))
}

/// Seat 0 controls a Watchful Fox that its upkeep trigger prepared, in the first main phase of its second turn.
fn fox_prepared(seats: usize) -> Result<(Driver, ObjectId), ScriptError> {
    let scenario = others(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(fox()),
            ready(mountain()),
            ready(mountain()),
            ready(plains()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let id = driver.id_of("Watchful Fox")?;
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
    Ok((driver, id))
}

#[rstest]
fn a_prepare_card_enters_prepared_and_the_log_says_so(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, id) = owl_entered(seats)?;
    assert!(prepared(&driver, id));
    let events = driver.engine().events();
    let entered = events
        .iter()
        .position(|event| matches!(event, Event::Entered { object, .. } if *object == id))
        .unwrap();
    assert_eq!(
        events
            .iter()
            .position(|e| *e == Event::Prepared { object: id }),
        Some(entered + 1),
        "the designation follows the entry"
    );
    Ok(())
}

#[rstest]
fn a_trigger_prepares_the_permanent_once(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, id) = fox_prepared(seats)?;
    assert!(prepared(&driver, id));
    let made = driver
        .engine()
        .events()
        .iter()
        .filter(|event| **event == Event::Prepared { object: id })
        .count();
    assert_eq!(made, 1);
    Ok(())
}

#[rstest]
fn the_controller_casts_the_spell_face_and_the_permanent_stops_being_prepared(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, id) = owl_entered(seats)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    let cards = driver.state().objects().count();
    driver.cast_prepared("Soothing Owl")?;
    // The copy is a spell named for the face; the permanent is still the Owl, no longer prepared.
    let stack = driver.state().stack().to_vec();
    assert_eq!(names(driver.state(), Zone::Stack), ["Soothing Song"]);
    assert!(driver.state().object(stack[0]).unwrap().is_spell_copy());
    assert!(!prepared(&driver, id));
    assert_eq!(
        sorted(names(driver.state(), Zone::Battlefield)),
        ["Plains", "Plains", "Soothing Owl"]
    );
    let events = driver.engine().events();
    let unprepared = events
        .iter()
        .position(|e| *e == Event::Unprepared { object: id })
        .unwrap();
    let cast = events
        .iter()
        .position(|e| matches!(e, Event::SpellCast { object, .. } if *object == stack[0]))
        .unwrap();
    assert!(unprepared < cast, "the cost of the cast unprepares it");
    resolve_top(&mut driver, seats)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life + 2);
    // The copy is no card: it ceased to exist and left nothing in any zone.
    assert_eq!(driver.state().objects().count(), cards);
    for zone in [Zone::Graveyard(pid(0)), Zone::Exile, Zone::Stack] {
        assert_eq!(
            names(driver.state(), zone),
            Vec::<String>::new(),
            "{zone:?}"
        );
    }
    assert!(!prepared(&driver, id));
    Ok(())
}

#[rstest]
fn the_face_is_not_offered_again_until_the_permanent_is_prepared_again(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, _) = owl_entered(seats)?;
    driver.cast_prepared("Soothing Owl")?;
    resolve_top(&mut driver, seats)?;
    assert!(driver.begin_prepared("Soothing Owl").is_err());
    Ok(())
}

#[rstest]
fn a_sorcery_face_waits_for_an_empty_stack_and_an_instant_face_does_not(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat 0 holds an Owl, which enters prepared: with its creature spell on the stack, the Owl's face is not there yet.
    let scenario = others(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![owl(), hawk()])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(plains()),
                ready(forest()),
                ready(forest()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Soothing Owl", &["Plains"])?;
    resolve_top(&mut driver, seats)?;
    driver.cast_tapping("Keen Hawk", &["Forest"])?;
    // Both are prepared now or soon; with the Hawk on the stack the sorcery face of the Owl is not castable, though it
    // is prepared.
    assert!(prepared(&driver, driver.id_of("Soothing Owl")?));
    assert!(driver.begin_prepared("Soothing Owl").is_err());
    resolve_top(&mut driver, seats)?;
    assert!(driver.begin_prepared("Keen Hawk").is_ok());
    Ok(())
}

#[rstest]
fn an_instant_face_is_cast_with_a_spell_on_the_stack(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, id) = fox_prepared(seats)?;
    driver.cast_tapping("Soothing Owl", &["Plains"]).ok();
    assert!(prepared(&driver, id));
    driver.begin_prepared("Watchful Fox")?;
    let target = driver.target_player(seats - 1);
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(names(driver.state(), Zone::Stack), ["Fire Bolt"]);
    assert!(!prepared(&driver, id));
    resolve_top(&mut driver, seats)?;
    assert_eq!(driver.state().player(pid(seats - 1)).unwrap().life(), 17);
    Ok(())
}

#[rstest]
fn an_opponent_cannot_cast_it_whatever_the_timing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, id) = fox_prepared(seats)?;
    // Seat 0 passes with the Fox prepared: no other seat is offered its face, and the action is refused for them.
    driver.pass()?;
    for _ in 1..seats {
        let decision = driver.decision()?.clone();
        assert_ne!(decision.player, pid(0));
        let DecisionKind::Priority(options) = &decision.kind else {
            break;
        };
        assert!(options.faces.iter().all(|face| face.card != id));
        let refused = driver.answer(Answer::Priority(PriorityAction::CastPrepared(id)));
        assert!(refused.is_err(), "seat {:?} cast it", decision.player);
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
fn backing_out_of_the_cast_keeps_the_permanent_prepared_and_the_copy_ceases(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, id) = owl_entered(seats)?;
    let cards = driver.state().objects().count();
    driver.begin_prepared("Soothing Owl")?;
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert!(prepared(&driver, id));
    assert_eq!(
        driver.state().objects().count(),
        cards,
        "the copy ceased to exist"
    );
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|e| matches!(e, Event::Unprepared { .. }))
    );
    // It can be cast now.
    driver.cast_prepared("Soothing Owl")?;
    assert!(!prepared(&driver, id));
    Ok(())
}

#[rstest]
fn a_countered_copy_leaves_the_permanent_unprepared(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, id) = owl_entered(seats)?;
    driver.cast_prepared("Soothing Owl")?;
    let copy = driver.state().stack()[0];
    // The copy leaves the stack with no resolution; the permanent does not get prepared again.
    let mut state = driver.state().clone();
    apply_event(
        &mut state,
        &Event::ZoneChanged {
            object: copy,
            from: Zone::Stack,
            to: Zone::Graveyard(pid(0)),
            cause: mtg_engine::events::ZoneCause::Countered,
        },
    )
    .unwrap();
    assert!(!state.object(id).unwrap().permanent().unwrap().prepared());
    assert_eq!(name_of(&state, id), "Soothing Owl");
    Ok(())
}

#[rstest]
fn a_prepared_permanent_that_leaves_the_battlefield_is_not_prepared_and_is_not_offered(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (driver, id) = owl_entered(seats)?;
    let mut state = driver.state().clone();
    apply_event(
        &mut state,
        &Event::CreatureDied {
            object: id,
            cause: mtg_engine::events::DeathCause::LethalDamage,
        },
    )
    .unwrap();
    assert!(state.object(id).unwrap().permanent().is_none());
    // Only a permanent on the battlefield becomes prepared, and only a prepared one stops being so.
    assert!(apply_event(&mut state, &Event::Prepared { object: id }).is_err());
    assert!(apply_event(&mut state, &Event::Unprepared { object: id }).is_err());
    Ok(())
}

#[rstest]
fn a_game_with_a_prepared_cast_replays_to_the_same_state(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, _) = owl_entered(seats)?;
    driver.cast_prepared("Soothing Owl")?;
    resolve_top(&mut driver, seats)?;
    let setup = driver.engine().setup().clone();
    let replayed = mtg_engine::replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
