//! Goal: a shield over a class of events (`Rule::Replaces`) is judged as each event happens, not for the objects there were
//! when it began: "if a creature would die this turn, exile it instead" exiles a creature that enters afterwards and
//! leaves lands and other zone changes alone, "cards put into your graveyard from anywhere are exiled" covers a discard,
//! and "permanents enter tapped this turn" taps a permanent that enters later; each shield applies once to a move.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::modify::Duration;
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_core::card::CardType;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use rstest::rstest;
use std::sync::Arc;

use super::{Processed, process};
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::rules::{Rule, RuleEffect};
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);

fn land(name: &str) -> Arc<mtg_cards::CardDefinition> {
    Arc::new(mtg_cards::CardDefinition::new(name).with_type(CardType::Land))
}

/// Seat 0 holds a Rat in hand and controls a Bear and a Forest; seat 1 controls an Ogre.
fn table() -> GameState {
    state(
        vec![vec![creature("Rat", 1, 1)], Vec::new()],
        vec![
            vec![creature("Bear", 2, 2), land("Forest")],
            vec![creature("Ogre", 3, 4)],
        ],
    )
}

fn id(game: &GameState, name: &str) -> ObjectId {
    game.objects()
        .find(|o| o.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

/// Puts the shield in force as a spell of the Bear's controller made it, until end of turn.
fn begin(game: &mut GameState, when: &str, by: &ReplaceBy) {
    let bear = game.object(id(game, "Bear")).unwrap();
    let effect = RuleEffect {
        id: EffectId(game.next_effect),
        rule: Rule::Replaces {
            when: serde_json::from_str::<ReplaceWhen>(when).unwrap(),
            by: by.clone(),
        },
        duration: Duration::UntilEndOfTurn,
        source: Some(bear.reference()),
        controller: bear.controller(),
        turn: 1,
    };
    apply_event(
        game,
        &Event::RuleCreated {
            effect: effect.into(),
        },
    )
    .unwrap();
}

/// What the move of `name` from one zone to another becomes after the shield.
fn moved(game: &GameState, name: &str, from: Zone, to: Zone) -> Vec<Event> {
    let batch = vec![Event::ZoneChanged {
        object: id(game, name),
        from,
        to,
        cause: ZoneCause::Moved,
    }];
    match process(game, batch, &[]).unwrap() {
        Processed::Events(events) => events,
        Processed::Ask { .. } => panic!("nothing is asked"),
    }
}

fn goes_to(events: &[Event]) -> Option<Zone> {
    events.iter().find_map(|event| match event {
        Event::ZoneChanged { to, .. } => Some(*to),
        _ => None,
    })
}

const CREATURES_DIE: &str =
    r#"{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}}"#;
const MY_CARDS_BURIED: &str =
    r#"{"ZoneChange":{"what":[{"Not":"Token"},{"OwnedBy":"You"}],"to":"Graveyard"}}"#;

#[rstest]
#[case::my_creature("Bear", Zone::Graveyard(ME), Zone::Exile)]
#[case::their_creature("Ogre", Zone::Graveyard(FOE), Zone::Exile)]
#[case::a_land_dies_as_usual("Forest", Zone::Graveyard(ME), Zone::Graveyard(ME))]
fn every_creature_that_would_die_is_exiled_instead(
    #[case] name: &str,
    #[case] to: Zone,
    #[case] becomes: Zone,
) {
    let mut game = table();
    begin(
        &mut game,
        CREATURES_DIE,
        &ReplaceBy::MoveTo(mtg_cards::refs::ZoneKind::Exile),
    );
    let events = moved(&game, name, Zone::Battlefield, to);
    assert_eq!(goes_to(&events), Some(becomes));
}

#[test]
fn a_creature_that_enters_after_the_shield_began_is_covered() {
    let mut game = table();
    begin(
        &mut game,
        CREATURES_DIE,
        &ReplaceBy::MoveTo(mtg_cards::refs::ZoneKind::Exile),
    );
    let rat = id(&game, "Rat");
    apply_event(
        &mut game,
        &Event::ZoneChanged {
            object: rat,
            from: Zone::Hand(ME),
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        },
    )
    .unwrap();
    let events = moved(&game, "Rat", Zone::Battlefield, Zone::Graveyard(ME));
    assert_eq!(goes_to(&events), Some(Zone::Exile));
}

#[rstest]
#[case::a_discard_from_hand("Rat", Zone::Hand(ME), Zone::Graveyard(ME), Zone::Exile)]
#[case::a_death("Bear", Zone::Battlefield, Zone::Graveyard(ME), Zone::Exile)]
#[case::not_a_card_of_theirs("Ogre", Zone::Battlefield, Zone::Graveyard(FOE), Zone::Graveyard(FOE))]
#[case::not_a_move_elsewhere("Bear", Zone::Battlefield, Zone::Hand(ME), Zone::Hand(ME))]
fn cards_put_into_my_graveyard_from_anywhere_are_exiled(
    #[case] name: &str,
    #[case] from: Zone,
    #[case] to: Zone,
    #[case] becomes: Zone,
) {
    let mut game = table();
    begin(
        &mut game,
        MY_CARDS_BURIED,
        &ReplaceBy::MoveTo(mtg_cards::refs::ZoneKind::Exile),
    );
    let events = moved(&game, name, from, to);
    assert_eq!(goes_to(&events), Some(becomes));
}

#[rstest]
#[case::a_permanent_that_enters(true)]
#[case::a_creature_is_a_permanent_too(false)]
fn permanents_enter_tapped_while_the_shield_lasts(#[case] land_enters: bool) {
    let mut game = table();
    begin(&mut game, r#"{"Enters":{"what":[]}}"#, &ReplaceBy::Tapped);
    let name = if land_enters { "Forest" } else { "Rat" };
    let object = id(&game, name);
    let batch = vec![
        Event::ZoneChanged {
            object,
            from: Zone::Hand(ME),
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        },
        Event::Entered {
            object,
            tapped: false,
            controller: ME,
            chosen: None,
        },
    ];
    let Processed::Events(events) = process(&game, batch, &[]).unwrap() else {
        panic!("nothing is asked");
    };
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Entered { tapped: true, .. })),
        "{events:?}"
    );
}
