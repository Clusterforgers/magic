//! Goal: the void tallies count a nonland permanent that left the battlefield (not a land, not a card from hand) and a spell cast with warp, the dungeon tally reads the completions of each player, and "attacked this combat" finds the creatures that attacked the reading player in this combat and no other.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::{Amount, CastWay, TallyEvent, TurnTally};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::apply_event;
use crate::abilities::Cause;
use crate::choose::{Choices, Target};
use crate::combat::AttackTarget;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::turn::Step;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);

fn land(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Land))
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

fn id(state: &GameState, name: &str) -> ObjectId {
    state
        .objects()
        .find(|o| o.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

fn fold(state: &mut GameState, events: &[Event]) {
    for event in events {
        apply_event(state, event).unwrap();
    }
}

fn read(state: &GameState, reader: PlayerId, amount: &Amount) -> i64 {
    let bear = state.object(id(state, "Bear")).unwrap().reference();
    Scope::announcing(state, reader, bear)
        .amount(amount)
        .unwrap()
}

fn tallied(state: &GameState, who: Ref, what: TurnTally) -> i64 {
    read(state, ME, &Amount::Tallied { who, what })
}

#[rstest]
#[case::a_creature_to_the_graveyard("Bear", Zone::Battlefield, Zone::Graveyard(ME), 1)]
#[case::a_creature_to_exile("Bear", Zone::Battlefield, Zone::Exile, 1)]
#[case::an_opponents_creature("Ogre", Zone::Battlefield, Zone::Graveyard(FOE), 1)]
#[case::a_land_is_no_void("Forest", Zone::Battlefield, Zone::Graveyard(ME), 0)]
#[case::a_card_from_hand_is_no_permanent("Rat", Zone::Hand(ME), Zone::Graveyard(ME), 0)]
fn only_a_nonland_permanent_leaving_the_battlefield_counts(
    #[case] name: &str,
    #[case] from: Zone,
    #[case] to: Zone,
    #[case] expected: i64,
) {
    let mut game = table();
    let leave = Event::ZoneChanged {
        object: id(&game, name),
        from,
        to,
        cause: ZoneCause::Moved,
    };
    fold(&mut game, &[leave]);
    let counted = tallied(&game, Ref::EachPlayer, TurnTally::NonlandPermanentsLeft);
    assert_eq!(counted, expected);
}

#[rstest]
#[case::warp(Some(CastWay::Warp), 1)]
#[case::hand_cast(None, 0)]
#[case::another_way(Some(CastWay::Overload), 0)]
fn only_a_spell_cast_with_warp_counts_as_warped(
    #[case] way: Option<CastWay>,
    #[case] expected: i64,
) {
    let mut game = table();
    let cast = Event::SpellCast {
        player: ME,
        object: id(&game, "Rat"),
        choices: Choices {
            way,
            ..Choices::default()
        },
    };
    fold(&mut game, &[cast]);
    let counted = tallied(&game, Ref::EachPlayer, TurnTally::SpellsWarped);
    assert_eq!(counted, expected);
}

#[rstest]
#[case::none(0, 0)]
#[case::one(1, 0)]
#[case::two_and_a_rival(2, 1)]
fn completed_dungeons_are_read_for_each_player(#[case] mine: u32, #[case] theirs: u32) {
    let mut game = table();
    game.w11d.completed.insert(ME, mine);
    game.w11d.completed.insert(FOE, theirs);
    let completed = |who| tallied(&game, who, TurnTally::DungeonsCompleted);
    assert_eq!(completed(Ref::You), i64::from(mine));
    assert_eq!(completed(Ref::Opponents), i64::from(theirs));
}

/// The creatures that attacked the reader in this combat: attacks counted, filtered to those that attacked "you".
fn attacked_you(state: &GameState, reader: PlayerId) -> i64 {
    let filter = Selector(vec![Pred::DidThisTurn {
        event: TallyEvent::Attacked,
        by: None,
        to: Some(Ref::You),
        combat: true,
    }]);
    let amount = Amount::Tally {
        event: TallyEvent::Attacked,
        who: Ref::EachPlayer,
        filter,
    };
    read(state, reader, &amount)
}

#[test]
fn a_player_has_been_attacked_only_by_creatures_that_attacked_them_this_combat() {
    let mut game = table();
    let begin = Event::StepBegan {
        step: Step::BeginCombat,
    };
    let attack = Event::AttackersDeclared {
        player: ME,
        attackers: vec![(id(&game, "Bear"), AttackTarget::Player(FOE))],
    };
    fold(&mut game, &[begin.clone(), attack]);
    assert_eq!(attacked_you(&game, FOE), 1);
    assert_eq!(attacked_you(&game, ME), 0);
    // A later combat phase starts afresh.
    fold(&mut game, &[begin]);
    assert_eq!(attacked_you(&game, FOE), 0);
}

#[test]
fn the_first_thing_a_trigger_is_about_is_one_object_not_all_of_them() {
    let game = table();
    let bear = game.object(id(&game, "Bear")).unwrap().reference();
    let object = |name: &str| Target::Object(game.object(id(&game, name)).unwrap().reference());
    let cause = Cause {
        subjects: vec![object("Bear"), object("Ogre"), Target::Player(FOE)],
        ..Cause::default()
    };
    let scope = Scope::triggered(&game, ME, bear, &cause);
    assert_eq!(scope.refs(&Ref::TriggeringFirst).unwrap(), [object("Bear")]);
    assert_eq!(scope.refs(&Ref::Triggering).unwrap().len(), 3);
    let empty = Cause::default();
    let scope = Scope::triggered(&game, ME, bear, &empty);
    assert!(scope.refs(&Ref::TriggeringFirst).unwrap().is_empty());
}
