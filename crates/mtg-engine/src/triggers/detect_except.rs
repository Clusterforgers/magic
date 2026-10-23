//! Goal: a zone-change pattern that names a zone it must not come from ("is put into a graveyard from anywhere other than the battlefield") triggers for every object that moves to the named zone from any other zone, and not for one that leaves the excluded zone, one that goes elsewhere or one the selector does not pick.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;

use super::detect_events::{ME, board, fired, id};
use crate::events::{Event, ZoneCause};
use crate::zones::Zone;

/// The pattern for a Watcher: an object matching `what` enters `to` from anywhere but `except`.
fn pattern_of(what: &str, except: &str, to: &str) -> String {
    let to = if to.is_empty() {
        String::new()
    } else {
        format!(r#","to":"{to}""#)
    };
    format!(r#"{{"ZoneChangeExcept":{{"what":{what},"except":"{except}"{to}}}}}"#)
}

/// Where each named object is at the start: the Spell in seat 0's hand, the Bear and the Cloak on the battlefield.
fn moves(name: &str, from: Zone, to: Zone) -> (&'static str, Zone, Zone) {
    let name = match name {
        "Spell" => "Spell",
        "Bear" => "Bear",
        _ => "Cloak",
    };
    (name, from, to)
}

#[rstest]
#[case::from_the_hand_to_a_graveyard(
    r"[]",
    "Battlefield",
    "Graveyard",
    moves("Spell", Zone::Hand(ME), Zone::Graveyard(ME)),
    1
)]
#[case::not_from_the_battlefield(
    r"[]",
    "Battlefield",
    "Graveyard",
    moves("Bear", Zone::Battlefield, Zone::Graveyard(ME)),
    0
)]
#[case::not_to_another_zone(
    r"[]",
    "Battlefield",
    "Graveyard",
    moves("Spell", Zone::Hand(ME), Zone::Exile),
    0
)]
#[case::from_the_battlefield_when_it_is_another_zone_excluded(
    r"[]",
    "Hand",
    "Graveyard",
    moves("Bear", Zone::Battlefield, Zone::Graveyard(ME)),
    1
)]
#[case::not_from_the_hand_when_the_hand_is_excluded(
    r"[]",
    "Hand",
    "Graveyard",
    moves("Spell", Zone::Hand(ME), Zone::Graveyard(ME)),
    0
)]
#[case::any_destination(
    r"[]",
    "Battlefield",
    "",
    moves("Spell", Zone::Hand(ME), Zone::Exile),
    1
)]
#[case::any_destination_not_from_the_excluded_zone(
    r"[]",
    "Battlefield",
    "",
    moves("Bear", Zone::Battlefield, Zone::Hand(ME)),
    0
)]
#[case::the_selector_picks(
    r#"[{"Type":"Creature"}]"#,
    "Battlefield",
    "Exile",
    moves("Spell", Zone::Hand(ME), Zone::Exile),
    0
)]
#[case::the_selector_picks_a_creature(
    r#"[{"Type":"Creature"}]"#,
    "Hand",
    "Exile",
    moves("Bear", Zone::Battlefield, Zone::Exile),
    1
)]
fn an_object_moving_to_a_zone_from_anywhere_but_one_triggers(
    #[case] what: &str,
    #[case] except: &str,
    #[case] to: &str,
    #[case] moved: (&str, Zone, Zone),
    #[case] expected: usize,
) {
    let pre = board(&pattern_of(what, except, to));
    let (name, from, to) = moved;
    let event = Event::ZoneChanged {
        object: id(&pre, name),
        from,
        to,
        cause: ZoneCause::Moved,
    };
    assert_eq!(fired(&pre, &[event]), expected);
}
