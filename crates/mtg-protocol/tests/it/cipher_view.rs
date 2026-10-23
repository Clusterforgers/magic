//! Goal: what cipher shows is public at tables of two, three and four seats: the card exiled encoded on a creature lies face up and every seat sees it with its name, the event that encodes it and the events that copy it and cast the copy reach every seat as logged, the copy is shown as a copy of the encoded card, and the hand of the player who owns them stays hidden from the other seats exactly as before.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{CardsChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::ZoneCause;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{CopyView, EventView, ObjectView, StateView, project_events};
use mtg_testkit::cards::{free_creature, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn view_of(state: &GameState, viewer: usize, id: ObjectId) -> Option<ObjectView> {
    StateView::for_player(state, PlayerId::from_index(viewer))
        .objects
        .into_iter()
        .find(|o| o.id == id)
}

fn named(state: &GameState, zone: Zone, name: &str) -> ObjectId {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .unwrap()
}

/// Seat 0 holds Writ of Return and Brainstorm, five Swamps and a ready creature, with a creature card in the graveyard.
fn table(seats: usize) -> Driver {
    let mut mine: Vec<_> = (0..5).map(|_| ready(swamp())).collect();
    mine.push(ready(free_creature("Carrier", 2, 2)));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(
            0,
            [card("Writ of Return").unwrap(), card("Brainstorm").unwrap()],
        )
        .graveyard(0, [free_creature("Corpse", 1, 1)])
        .battlefield(0, mine);
    let scenario = (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| swamp()))
    });
    Driver::from_scenario(scenario).unwrap()
}

/// Answers decisions until `stop` holds at a decision of priority; the copy is cast when `cast`.
fn play_until(driver: &mut Driver, cast: bool, stop: impl Fn(&Driver) -> bool) {
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if stop(driver) => return,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
                continue;
            }
            DecisionKind::ChooseCards(options) => {
                let state = driver.state();
                let carrier = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| state.object(*id).unwrap().card().name == "Carrier")
                    .collect();
                Answer::ChooseCards(CardsChoice(carrier))
            }
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(cast)),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseTargets(options) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![options.legal[0]]))
            }
            _ => {
                driver.answer_passively().unwrap();
                continue;
            }
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game did not reach the stop");
}

/// Seat 0 casts Writ of Return and encodes it on the creature: the state before the spell resolves, the log from there
/// and the exiled card.
fn encoded(seats: usize) -> (Driver, ObjectId, GameState, Vec<Event>) {
    let mut driver = table(seats);
    let corpse = driver.target_named("Corpse");
    let target = corpse.unwrap_or_else(|_| driver.target_player(0));
    driver.begin_cast("Writ of Return").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let before = driver.state().clone();
    let from = driver.engine().events().len();
    play_until(&mut driver, true, |driver| {
        driver.state().stack().is_empty()
    });
    let events = driver.engine().events()[from..].to_vec();
    let exiled = named(driver.state(), Zone::Exile, "Writ of Return");
    (driver, exiled, before, events)
}

#[rstest]
fn the_encoded_card_lies_face_up_and_every_seat_sees_it(#[values(2, 3, 4)] seats: usize) {
    let (driver, exiled, ..) = encoded(seats);
    for viewer in 0..seats {
        let shown = view_of(driver.state(), viewer, exiled)
            .unwrap_or_else(|| panic!("seat {viewer} sees the encoded card"));
        assert_eq!(shown.name, "Writ of Return");
        assert_eq!(shown.zone, Zone::Exile);
        assert_eq!(shown.kind, ObjectKind::Card);
    }
}

#[rstest]
fn the_hand_of_the_player_who_encoded_stays_hidden(#[values(2, 3, 4)] seats: usize) {
    let (driver, ..) = encoded(seats);
    let hand = driver.state().zone(Zone::Hand(PlayerId(0))).to_vec();
    assert_eq!(hand.len(), 1, "Brainstorm is left");
    assert!(view_of(driver.state(), 0, hand[0]).is_some());
    for viewer in 1..seats {
        assert!(
            view_of(driver.state(), viewer, hand[0]).is_none(),
            "seat {viewer} does not see the card"
        );
    }
}

#[rstest]
fn the_event_that_encodes_the_card_reaches_every_seat_as_logged(#[values(2, 3, 4)] seats: usize) {
    let (driver, exiled, before, events) = encoded(seats);
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    let carrier = named(driver.state(), Zone::Battlefield, "Carrier");
    for (viewer, batch) in batches.iter().enumerate() {
        let seen = batch.events.iter().any(|view| {
            matches!(
                view,
                EventView::Public(Event::ZoneChanged {
                    object,
                    to: Zone::Exile,
                    cause: ZoneCause::Encoded { by },
                    ..
                }) if *object == exiled && by.id == carrier
            )
        });
        assert!(seen, "seat {viewer} sees the card being encoded");
        assert!(batch.objects.iter().any(|o| o.id == exiled));
    }
}

/// The copy is on the stack: the state before the question, the log from there and the copy.
fn with_a_copy(seats: usize) -> (Driver, ObjectId, ObjectId, GameState, Vec<Event>) {
    let (mut driver, exiled, ..) = encoded(seats);
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Carrier", 1)]).unwrap();
    // The trigger waits on the stack until the players pass; the copy is asked as it resolves.
    for _ in 0..40 {
        if matches!(driver.decision().unwrap().kind, DecisionKind::YesNo(_)) {
            break;
        }
        driver.answer_passively().unwrap();
    }
    let before = driver.state().clone();
    let from = driver.engine().events().len();
    driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
    for _ in 0..10 {
        if driver.state().stack().len() > 1 {
            break;
        }
        play_until(&mut driver, true, |driver| driver.state().stack().len() > 1);
    }
    let events = driver.engine().events()[from..].to_vec();
    let copy = *driver.state().stack().last().unwrap();
    (driver, exiled, copy, before, events)
}

#[rstest]
fn the_copy_is_shown_to_every_seat_as_a_copy_of_the_encoded_card(#[values(2, 3, 4)] seats: usize) {
    let (driver, exiled, copy, ..) = with_a_copy(seats);
    assert!(driver.state().object(copy).unwrap().is_spell_copy());
    for viewer in 0..seats {
        let shown = view_of(driver.state(), viewer, copy).unwrap();
        assert_eq!(shown.name, "Writ of Return");
        assert_eq!(shown.zone, Zone::Stack);
        assert_eq!(shown.kind, ObjectKind::Token);
        assert_eq!(shown.controller, PlayerId(0));
        assert_eq!(shown.copy, Some(CopyView { of: Some(exiled) }));
    }
}

#[rstest]
fn the_events_of_the_copy_are_public(#[values(2, 3, 4)] seats: usize) {
    let (_, exiled, copy, before, events) = with_a_copy(seats);
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in batches.iter().enumerate() {
        let made = batch.events.iter().find_map(|view| match view {
            EventView::Public(Event::CardCopied(made)) => Some((made.object, made.of.id)),
            _ => None,
        });
        assert_eq!(made, Some((copy, exiled)), "seat {viewer} sees the copy");
        let cast = batch.events.iter().any(|view| {
            matches!(view, EventView::Public(Event::SpellCast { object, .. }) if *object == copy)
        });
        assert!(cast, "seat {viewer} sees the copy being cast");
        assert!(batch.objects.iter().any(|o| o.id == copy));
    }
}

#[rstest]
fn a_copy_that_is_not_cast_ceases_to_exist_in_every_view(#[values(2, 3, 4)] seats: usize) {
    let (mut driver, ..) = encoded(seats);
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Carrier", 1)]).unwrap();
    for _ in 0..40 {
        if matches!(driver.decision().unwrap().kind, DecisionKind::YesNo(_)) {
            break;
        }
        driver.answer_passively().unwrap();
    }
    let before = driver.state().clone();
    let from = driver.engine().events().len();
    driver.answer(Answer::YesNo(YesNoChoice(false))).unwrap();
    play_until(&mut driver, false, |driver| {
        driver.state().stack().is_empty()
    });
    let events = driver.engine().events()[from..].to_vec();
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in batches.iter().enumerate() {
        let ceased = batch
            .events
            .iter()
            .any(|view| matches!(view, EventView::Public(Event::CopyCeased { .. })));
        assert!(ceased, "seat {viewer} sees the copy cease to exist");
        assert!(batch.gone.iter().any(|id| {
            before
                .object(*id)
                .is_some_and(|o| o.kind() == ObjectKind::Token)
        }));
    }
}
