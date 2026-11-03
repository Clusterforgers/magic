//! Goal: `Did { action, who, what }` triggers on the event of each action it names for the players and objects it names, never on another action or on the same action of someone else, hands the ability the player and the object as subjects, and judges what left the battlefield as it was there.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::detect;
use super::detect_events::{FOE, ME, board, id};
use crate::choose::Target;
use crate::events::{Event, ZoneCause};
use crate::library::{Scried, Surveilled};
use crate::player::LossReason;
use crate::state::GameState;
use crate::zones::Zone;

/// The events one action of `who` on the object named `name` is, on the `board`.
type Made = fn(&GameState, PlayerId, ObjectId) -> Vec<Event>;

fn moved(cause: ZoneCause) -> impl Fn(&GameState, ObjectId) -> Event {
    move |state, object| {
        let from = state.object(object).unwrap().zone();
        Event::ZoneChanged {
            object,
            from,
            to: Zone::Graveyard(PlayerId(1)),
            cause,
        }
    }
}

fn destroyed(state: &GameState, _: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![moved(ZoneCause::Destroyed)(state, object)]
}

fn buried(state: &GameState, _: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![moved(ZoneCause::Buried)(state, object)]
}

fn exiled(state: &GameState, _: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![moved(ZoneCause::Exiled)(state, object)]
}

fn sacrificed(state: &GameState, _: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![moved(ZoneCause::Sacrificed)(state, object)]
}

fn countered(state: &GameState, _: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![moved(ZoneCause::Countered)(state, object)]
}

fn milled(state: &GameState, _: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![moved(ZoneCause::Milled)(state, object)]
}

fn foretold(state: &GameState, _: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![moved(ZoneCause::Foretold)(state, object)]
}

fn moved_plainly(state: &GameState, _: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![moved(ZoneCause::Moved)(state, object)]
}

fn scried(_: &GameState, player: PlayerId, _: ObjectId) -> Vec<Event> {
    vec![Event::Scried(Scried {
        player,
        top: Vec::new(),
        bottom: Vec::new(),
    })]
}

fn surveilled(_: &GameState, player: PlayerId, _: ObjectId) -> Vec<Event> {
    vec![Event::Surveilled(Surveilled {
        player,
        top: Vec::new(),
        graveyard: Vec::new(),
    })]
}

fn shuffled(_: &GameState, player: PlayerId, _: ObjectId) -> Vec<Event> {
    vec![Event::LibraryShuffled {
        player,
        order: Vec::new(),
    }]
}

fn lost(_: &GameState, player: PlayerId, _: ObjectId) -> Vec<Event> {
    vec![Event::PlayerLost {
        player,
        reason: LossReason::Conceded,
    }]
}

fn played_land(_: &GameState, player: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![Event::LandPlayed {
        player,
        object,
        face: None,
    }]
}

fn transformed(_: &GameState, _: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![Event::Transformed { object, face: None }]
}

fn unattached(_: &GameState, _: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![Event::Unattached { object }]
}

fn explored(_: &GameState, player: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![Event::Explored { object, player }]
}

fn connived(_: &GameState, player: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![Event::Connived { object, player }]
}

fn monstrous(_: &GameState, player: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![Event::Monstrous {
        object,
        player,
        x: 3,
    }]
}

fn learned(_: &GameState, player: PlayerId, _: ObjectId) -> Vec<Event> {
    vec![Event::Learned { player }]
}

fn proliferated(_: &GameState, player: PlayerId, _: ObjectId) -> Vec<Event> {
    vec![Event::Proliferated { player }]
}

fn unlocked(_: &GameState, player: PlayerId, object: ObjectId) -> Vec<Event> {
    vec![Event::DoorUnlocked {
        object,
        door: 0,
        player,
    }]
}

/// The abilities `events` trigger, judged against the board as it is.
fn count(pre: &GameState, events: &[Event]) -> Vec<crate::triggers::Triggered> {
    detect(pre, events, pre)
}

#[rstest]
#[case::played_land(r#"{"action":"PlayedLand"}"#, played_land as Made, ME, "Forest", 1)]
#[case::a_land_of_mine(r#"{"action":"PlayedLand","who":"You"}"#, played_land, ME, "Forest", 1)]
#[case::a_land_of_theirs_is_not_mine(
    r#"{"action":"PlayedLand","who":"You"}"#,
    played_land,
    FOE,
    "Their Forest",
    0
)]
#[case::an_opponents_land(
    r#"{"action":"PlayedLand","who":"Opponents"}"#,
    played_land,
    FOE,
    "Their Forest",
    1
)]
#[case::a_land_not_a_creature(
    r#"{"action":"PlayedLand","what":[{"Type":"Creature"}]}"#,
    played_land,
    ME,
    "Forest",
    0
)]
#[case::transformed(
    r#"{"action":"Transformed","what":[{"Type":"Creature"}]}"#,
    transformed,
    ME,
    "Bear",
    1
)]
#[case::transformed_is_not_unattached(r#"{"action":"Unattached"}"#, transformed, ME, "Bear", 0)]
#[case::unattached(
    r#"{"action":"Unattached","what":[{"Subtype":"Aura"}]}"#,
    unattached,
    ME,
    "Cloak",
    1
)]
#[case::scried(r#"{"action":"Scried","who":"You"}"#, scried, ME, "Bear", 1)]
#[case::scried_by_them(r#"{"action":"Scried","who":"You"}"#, scried, FOE, "Bear", 0)]
#[case::scried_by_an_opponent(r#"{"action":"Scried","who":"Opponents"}"#, scried, FOE, "Bear", 1)]
#[case::scry_with_an_object_filter_has_no_object(
    r#"{"action":"Scried","what":[{"Type":"Creature"}]}"#,
    scried,
    ME,
    "Bear",
    0
)]
#[case::surveilled(r#"{"action":"Surveilled"}"#, surveilled, FOE, "Bear", 1)]
#[case::shuffled(r#"{"action":"Shuffled","who":"Opponents"}"#, shuffled, FOE, "Bear", 1)]
#[case::lost_the_game(r#"{"action":"LostGame","who":"Opponents"}"#, lost, FOE, "Bear", 1)]
#[case::i_lost_the_game(r#"{"action":"LostGame","who":"Opponents"}"#, lost, ME, "Bear", 0)]
#[case::destroyed(
    r#"{"action":"Destroyed","what":[{"Type":"Creature"}]}"#,
    destroyed,
    FOE,
    "Ogre",
    1
)]
#[case::buried_is_destroyed(r#"{"action":"Destroyed"}"#, buried, FOE, "Ogre", 1)]
#[case::destroyed_what_i_control(r#"{"action":"Destroyed","who":"You"}"#, destroyed, ME, "Bear", 1)]
#[case::destroyed_what_they_control_is_not_mine(
    r#"{"action":"Destroyed","who":"You"}"#,
    destroyed,
    FOE,
    "Ogre",
    0
)]
#[case::destroyed_is_not_exiled(r#"{"action":"Exiled"}"#, destroyed, FOE, "Ogre", 0)]
#[case::exiled(r#"{"action":"Exiled","who":"Opponents"}"#, exiled, FOE, "Ogre", 1)]
#[case::exiled_is_not_sacrificed(r#"{"action":"Sacrificed"}"#, exiled, FOE, "Ogre", 0)]
#[case::sacrificed(r#"{"action":"Sacrificed","who":"You"}"#, sacrificed, ME, "Bear", 1)]
#[case::countered(r#"{"action":"Countered"}"#, countered, ME, "Bear", 1)]
#[case::milled(r#"{"action":"Milled"}"#, milled, ME, "Bear", 1)]
#[case::foretold(r#"{"action":"Foretold","who":"You"}"#, foretold, ME, "Bear", 1)]
#[case::a_plain_move_is_no_action(r#"{"action":"Destroyed"}"#, moved_plainly, ME, "Bear", 0)]
#[case::explored(
    r#"{"action":"Explored","what":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
    explored,
    ME,
    "Bear",
    1
)]
#[case::explored_by_them(
    r#"{"action":"Explored","what":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
    explored,
    FOE,
    "Ogre",
    0
)]
#[case::connived(r#"{"action":"Connived"}"#, connived, FOE, "Ogre", 1)]
#[case::monstrous(
    r#"{"action":"BecameMonstrous","what":["This"]}"#,
    monstrous,
    ME,
    "Watcher",
    1
)]
#[case::monstrous_is_another_creature(
    r#"{"action":"BecameMonstrous","what":["This"]}"#,
    monstrous,
    ME,
    "Bear",
    0
)]
#[case::learned(r#"{"action":"Learned","who":"You"}"#, learned, ME, "Bear", 1)]
#[case::proliferated(
    r#"{"action":"Proliferated","who":"Opponents"}"#,
    proliferated,
    FOE,
    "Bear",
    1
)]
#[case::a_door_unlocked(r#"{"action":"Unlocked"}"#, unlocked, ME, "Bear", 1)]
#[case::a_room_not_fully_unlocked(r#"{"action":"FullyUnlocked"}"#, unlocked, ME, "Bear", 0)]
fn an_action_triggers_what_waits_for_it_for_the_player_and_object_it_names(
    #[case] did: &str,
    #[case] make: Made,
    #[case] player: PlayerId,
    #[case] object: &str,
    #[case] expected: usize,
) {
    let when = format!(r#"{{"Did":{did}}}"#);
    let pre = board(&when);
    let object = id(&pre, object);
    // The events are not folded: they name what the board already holds.
    let events = make(&pre, player, object);
    let fired = count(&pre, &events);
    assert_eq!(fired.len(), expected, "{events:?}");
}

#[rstest]
#[case::player_then_object(played_land as Made, "Forest", 2)]
#[case::a_player_alone(scried, "Forest", 1)]
fn the_subjects_are_the_player_and_the_object_when_the_action_has_them(
    #[case] make: Made,
    #[case] object: &str,
    #[case] subjects: usize,
) {
    let action = if subjects == 2 {
        "PlayedLand"
    } else {
        "Scried"
    };
    let pre = board(&format!(r#"{{"Did":{{"action":"{action}"}}}}"#));
    let object = id(&pre, object);
    let found = count(&pre, &make(&pre, ME, object));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].cause.subjects.len(), subjects);
    assert_eq!(found[0].cause.subjects[0], Target::Player(ME));
}
