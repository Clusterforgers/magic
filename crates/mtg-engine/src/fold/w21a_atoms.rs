//! Goal: each open predicate atom holds exactly for the objects the game's facts make it true of: `Modified` (a counter, an Equipment, an Aura its controller controls; not an opponent's Aura), `FaceDown`, `TargetsPlayer` (a spell on the stack with that player as a target), `GravedThisTurn` (a card put into a graveyard from the battlefield this turn, not one discarded) and the `CardsLeftGraveyard` tally of the owner, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::CardDefinition;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;
use std::sync::Arc;

use super::apply_event;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::object::FaceDown;
use crate::resolve::Scope;
use crate::state::{GameState, Tally};
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

fn artifact(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_subtype(subtype),
    )
}

fn enchantment(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_subtype(subtype),
    )
}

/// Seat 0 holds Bear, Wolf, Elk, Owl, Sword, Shield Aura and Wrath on the battlefield and Rat in hand; seat 1 a Curse Aura;
/// each seat a Guest.
fn table(seats: usize) -> GameState {
    let hand = |seat: usize| {
        if seat == 0 {
            vec![creature("Rat", 1, 1), creature("Bolt", 1, 1)]
        } else {
            Vec::new()
        }
    };
    let field = |seat: usize| {
        let mut cards = vec![creature(&format!("Guest {seat}"), 2, 2)];
        if seat == 0 {
            cards.extend([
                creature("Bear", 2, 2),
                creature("Wolf", 2, 2),
                creature("Elk", 2, 2),
                creature("Owl", 1, 1),
                creature("Plain", 1, 1),
                artifact("Sword", "Equipment"),
                enchantment("Shield", "Aura"),
            ]);
        }
        if seat == 1 {
            cards.push(enchantment("Curse", "Aura"));
        }
        cards
    };
    state(
        (0..seats).map(hand).collect(),
        (0..seats).map(field).collect(),
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

fn holds(state: &GameState, pred: Pred, name: &str) -> bool {
    let object = state.object(id(state, name)).unwrap().reference();
    let source = state.object(id(state, "Guest 0")).unwrap().reference();
    Scope::announcing(state, ME, source)
        .selects(&Selector(vec![pred]), object)
        .unwrap()
}

/// The table after the facts each atom reads: a counter on Bear, Sword on Wolf, Shield on Elk, the opponent's Curse on
/// Owl; Plain stays face up here.
fn modified_table(seats: usize) -> GameState {
    let mut game = table(seats);
    let bear = game.object(id(&game, "Bear")).unwrap().reference();
    let events = [
        Event::CountersChanged(CountersChanged {
            on: Target::Object(bear),
            counter: CounterKind::PlusOnePlusOne,
            delta: 1,
        }),
        Event::Attached {
            object: id(&game, "Sword"),
            to: id(&game, "Wolf"),
        },
        Event::Attached {
            object: id(&game, "Shield"),
            to: id(&game, "Elk"),
        },
        Event::Attached {
            object: id(&game, "Curse"),
            to: id(&game, "Owl"),
        },
    ];
    fold(&mut game, &events);
    game
}

#[rstest]
#[case::a_counter(Pred::Modified, "Bear", true)]
#[case::equipped(Pred::Modified, "Wolf", true)]
#[case::own_aura(Pred::Modified, "Elk", true)]
#[case::an_opponents_aura(Pred::Modified, "Owl", false)]
#[case::nothing(Pred::Modified, "Guest 0", false)]
#[case::face_up(Pred::FaceDown, "Bear", false)]
#[case::graved_on_the_battlefield(Pred::GravedThisTurn, "Bear", false)]
fn an_atom_holds_for_the_objects_the_facts_make_it_true_of(
    #[values(2, 3, 4)] seats: usize,
    #[case] pred: Pred,
    #[case] name: &str,
    #[case] expected: bool,
) {
    let game = modified_table(seats);
    assert_eq!(holds(&game, pred, name), expected, "{name}");
}

#[rstest]
fn a_card_put_into_a_graveyard_from_the_battlefield_is_graved_and_a_discarded_one_is_not(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut game = table(seats);
    let events = [
        Event::ZoneChanged {
            object: id(&game, "Bear"),
            from: Zone::Battlefield,
            to: Zone::Graveyard(ME),
            cause: ZoneCause::Moved,
        },
        Event::ZoneChanged {
            object: id(&game, "Rat"),
            from: Zone::Hand(ME),
            to: Zone::Graveyard(ME),
            cause: ZoneCause::Moved,
        },
    ];
    fold(&mut game, &events);
    assert!(holds(&game, Pred::GravedThisTurn, "Bear"));
    assert!(!holds(&game, Pred::GravedThisTurn, "Rat"));
    assert_eq!(game.tallied(ME, Tally::W21aLeftGraveyard), 0);
    let back = Event::ZoneChanged {
        object: id(&game, "Rat"),
        from: Zone::Graveyard(ME),
        to: Zone::Hand(ME),
        cause: ZoneCause::Moved,
    };
    fold(&mut game, &[back]);
    assert_eq!(game.tallied(ME, Tally::W21aLeftGraveyard), 1);
    for seat in 1..seats {
        let player = PlayerId(u64::try_from(seat).unwrap());
        assert_eq!(game.tallied(player, Tally::W21aLeftGraveyard), 0);
    }
}

#[rstest]
fn a_face_down_permanent_is_face_down_and_the_rest_are_not(#[values(2, 3, 4)] seats: usize) {
    let mut game = table(seats);
    let plain = id(&game, "Plain");
    game.objects.get_mut(&plain).unwrap().face_down = Some(FaceDown::Morph);
    let source = game.object(id(&game, "Guest 0")).unwrap().reference();
    let scope = Scope::announcing(&game, ME, source);
    let face_down = Selector(vec![Pred::FaceDown]);
    let reference = game.object(plain).unwrap().reference();
    assert!(scope.selects(&face_down, reference).unwrap());
    for other in game.objects().filter(|object| object.id() != plain) {
        assert!(!scope.selects(&face_down, other.reference()).unwrap());
    }
}

#[rstest]
#[case::targets_me(Ref::You, true)]
#[case::targets_them(Ref::Opponents, false)]
fn a_spell_targets_the_player_it_names(
    #[values(2, 3, 4)] seats: usize,
    #[case] who: Ref,
    #[case] expected: bool,
) {
    let mut game = table(seats);
    let bolt = id(&game, "Bolt");
    let cast = Event::ZoneChanged {
        object: bolt,
        from: Zone::Hand(ME),
        to: Zone::Stack,
        cause: ZoneCause::Moved,
    };
    fold(&mut game, &[cast]);
    game.objects.get_mut(&bolt).unwrap().choices.targets = vec![vec![Target::Player(ME)]];
    assert_eq!(holds(&game, Pred::TargetsPlayer(who), "Bolt"), expected);
    assert!(!holds(&game, Pred::TargetsPlayer(Ref::You), "Bear"));
}
