//! Goal: a Vehicle that becomes crewed and a Mount that becomes saddled trigger what watches them (the source itself or the permanents the pattern names, of any controller), a crew never answers a saddle pattern and the other way round, the cause names the controller and the permanent, and "for the first time each turn" counts each permanent's crews and saddles apart, restarts with the turn and with a permanent that changed zones, and counts the events of one batch in order, at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::detect_events::{FOE, ME, fired, fires, fold, id, pattern};
use crate::choose::Target;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::state::GameState;
use crate::zones::Zone;

/// A creature that gains its controller 1 life whenever `when`.
fn watcher(name: &str, when: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Triggered {
                when: pattern(when),
                intervening_if: None,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: vec![Effect::GainLife {
                    who: Ref::You,
                    amount: Amount::Fixed(1),
                }],
            }),
    )
}

/// `seats` players; seat 0 controls the Watcher of `when`, a Rig that watches for nothing and a Bear, the others a Guest.
fn table(seats: usize, when: &str) -> GameState {
    let field = |seat: usize| {
        if seat == 0 {
            vec![
                watcher("Watcher", when),
                creature("Rig", 3, 3),
                creature("Bear", 2, 2),
            ]
        } else {
            vec![creature(&format!("Guest {seat}"), 2, 2)]
        }
    };
    state(
        (0..seats).map(|_| Vec::new()).collect(),
        (0..seats).map(field).collect(),
    )
}

/// `name` is crewed (saddled) by the creatures `by`, under the control of `player`.
fn crewed(pre: &GameState, name: &str, by: &[&str], saddle: bool) -> Event {
    let player = pre.object(id(pre, name)).unwrap().controller();
    Event::Crewed {
        object: id(pre, name),
        player,
        creatures: by.iter().map(|creature| id(pre, creature)).collect(),
        saddle,
    }
}

const THIS_CREWED: &str = r#"{"BecomesCrewed":{"what":["This"]}}"#;
const THIS_SADDLED: &str = r#"{"BecomesSaddled":{"what":["This"]}}"#;
const ANY_CREWED: &str = r#"{"BecomesCrewed":{"what":[]}}"#;
const MINE_CREWED: &str = r#"{"BecomesCrewed":{"what":[{"ControlledBy":"You"}]}}"#;
const MINE_SADDLED: &str = r#"{"BecomesSaddled":{"what":[{"ControlledBy":"You"}]}}"#;

#[rstest]
#[case::it_is_crewed(THIS_CREWED, "Watcher", false, 1)]
#[case::another_rig_is_not_the_watcher(THIS_CREWED, "Rig", false, 0)]
#[case::a_saddle_is_not_a_crew(THIS_CREWED, "Watcher", true, 0)]
#[case::it_is_saddled(THIS_SADDLED, "Watcher", true, 1)]
#[case::another_mount_is_not_the_watcher(THIS_SADDLED, "Rig", true, 0)]
#[case::a_crew_is_not_a_saddle(THIS_SADDLED, "Watcher", false, 0)]
#[case::any_vehicle_of_mine(MINE_CREWED, "Rig", false, 1)]
#[case::a_vehicle_of_theirs_is_not_mine(MINE_CREWED, "Guest 1", false, 0)]
#[case::any_vehicle_of_theirs_when_anyones(ANY_CREWED, "Guest 1", false, 1)]
#[case::any_mount_of_mine(MINE_SADDLED, "Bear", true, 1)]
#[case::a_mount_of_theirs_is_not_mine(MINE_SADDLED, "Guest 1", true, 0)]
fn becoming_crewed_or_saddled_triggers_for_the_permanents_the_pattern_names(
    #[case] when: &str,
    #[case] name: &str,
    #[case] saddle: bool,
    #[case] expected: usize,
    #[values(2, 3, 4)] seats: usize,
) {
    let pre = table(seats, when);
    let event = crewed(&pre, name, &["Bear"], saddle);
    assert_eq!(fired(&pre, &[event]), expected);
}

#[rstest]
#[case::mine("Watcher", ME)]
#[case::theirs("Guest 1", FOE)]
fn the_cause_names_the_controller_and_the_permanent(
    #[case] name: &str,
    #[case] controller: PlayerId,
    #[values(2, 3, 4)] seats: usize,
) {
    let pre = table(seats, ANY_CREWED);
    let found = fires(&pre, &[crewed(&pre, name, &["Bear"], false)]);
    assert_eq!(found.len(), 1);
    let object = pre.object(id(&pre, name)).unwrap().reference();
    assert_eq!(
        found[0].cause.subjects,
        vec![Target::Player(controller), Target::Object(object)]
    );
}

fn first_crew(what: &str) -> String {
    format!(r#"{{"Nth":{{"event":{{"BecomesCrewed":{{"what":{what}}}}},"nth":1}}}}"#)
}

fn first_saddle(what: &str) -> String {
    format!(r#"{{"Nth":{{"event":{{"BecomesSaddled":{{"what":{what}}}}},"nth":1}}}}"#)
}

#[rstest]
#[case::the_first_crew(&[], ("Watcher", false), 1)]
#[case::not_the_second_crew(&[("Watcher", false)], ("Watcher", false), 0)]
#[case::not_the_third_crew(&[("Watcher", false), ("Watcher", false)], ("Watcher", false), 0)]
#[case::a_saddle_does_not_use_it_up(&[("Watcher", true)], ("Watcher", false), 1)]
fn the_first_crew_each_turn_is_counted_for_the_vehicle(
    #[case] before: &[(&str, bool)],
    #[case] last: (&str, bool),
    #[case] expected: usize,
    #[values(2, 3, 4)] seats: usize,
) {
    let pre = table(seats, &first_crew(r#"["This"]"#));
    let mut now = pre.clone();
    for (name, saddle) in before {
        now = fold(&now, &[crewed(&pre, name, &["Bear"], *saddle)]);
    }
    assert_eq!(
        fired(&now, &[crewed(&pre, last.0, &["Bear"], last.1)]),
        expected
    );
}

#[rstest]
#[case::the_first_saddle(&[], ("Watcher", true), 1)]
#[case::not_the_second_saddle(&[("Watcher", true)], ("Watcher", true), 0)]
#[case::a_crew_does_not_use_it_up(&[("Watcher", false)], ("Watcher", true), 1)]
fn the_first_saddle_each_turn_is_counted_for_the_mount(
    #[case] before: &[(&str, bool)],
    #[case] last: (&str, bool),
    #[case] expected: usize,
    #[values(2, 3, 4)] seats: usize,
) {
    let pre = table(seats, &first_saddle(r#"["This"]"#));
    let mut now = pre.clone();
    for (name, saddle) in before {
        now = fold(&now, &[crewed(&pre, name, &["Bear"], *saddle)]);
    }
    assert_eq!(
        fired(&now, &[crewed(&pre, last.0, &["Bear"], last.1)]),
        expected
    );
}

#[test]
fn each_vehicle_counts_its_own_crews_and_the_batch_is_counted_in_order() {
    let pre = table(2, &first_crew("[]"));
    let rig = crewed(&pre, "Rig", &["Bear"], false);
    let watcher = crewed(&pre, "Watcher", &["Bear"], false);
    let guest = crewed(&pre, "Guest 1", &["Guest 1"], false);
    // Three Vehicles, each crewed for the first time: the Watcher's pattern names any of them, so it triggers three times.
    assert_eq!(fired(&pre, &[rig.clone(), watcher.clone(), guest]), 3);
    // The same Vehicle twice in one batch is the first time once.
    assert_eq!(fired(&pre, &[watcher.clone(), watcher.clone()]), 1);
    // The Rig's crew does not use up the Watcher's first.
    let after_rig = fold(&pre, &[rig]);
    assert_eq!(fired(&after_rig, &[watcher]), 1);
}

#[test]
fn a_new_turn_starts_the_count_again() {
    let pre = table(2, &first_crew(r#"["This"]"#));
    let crew = crewed(&pre, "Watcher", &["Bear"], false);
    let first = fold(&pre, std::slice::from_ref(&crew));
    assert_eq!(fired(&first, std::slice::from_ref(&crew)), 0, "the second");
    let turn = Event::TurnBegan {
        number: 2,
        active: FOE,
        extra: false,
    };
    let next = fold(&first, &[turn]);
    assert_eq!(fired(&next, &[crew]), 1, "the first of the new turn");
}

#[test]
fn a_permanent_that_changed_zones_is_a_new_object_and_counts_from_the_start() {
    let pre = table(2, &first_crew(r#"["This"]"#));
    let object = id(&pre, "Watcher");
    let first = fold(&pre, &[crewed(&pre, "Watcher", &["Bear"], false)]);
    let leave = Event::ZoneChanged {
        object,
        from: Zone::Battlefield,
        to: Zone::Hand(ME),
        cause: ZoneCause::Moved,
    };
    let back = Event::ZoneChanged {
        object,
        from: Zone::Hand(ME),
        to: Zone::Battlefield,
        cause: ZoneCause::Moved,
    };
    let returned = fold(&first, &[leave, back]);
    assert_eq!(
        fired(&returned, &[crewed(&returned, "Watcher", &["Bear"], false)]),
        1
    );
}
