//! Goal: `Amount::Tally` over `Entered` and `Died` counts the permanents that came onto the battlefield or went to a graveyard from it this turn, as they were then, so one that left or returned since still counts, and the next turn clears both.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::{Amount, TallyEvent};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::apply_event;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

/// Seat 0 holds a Newcomer and has an Old creature in play; seat 1 has a Guest in play.
fn table() -> GameState {
    state(
        vec![vec![creature("Newcomer", 1, 1)], Vec::new()],
        vec![vec![creature("Old", 2, 2)], vec![creature("Guest", 2, 2)]],
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

fn moved(state: &GameState, name: &str, from: Zone, to: Zone) -> Event {
    Event::ZoneChanged {
        object: id(state, name),
        from,
        to,
        cause: ZoneCause::Moved,
    }
}

fn tally(state: &GameState, event: TallyEvent, filter: Vec<Pred>) -> i64 {
    let old = state.object(id(state, "Old")).unwrap().reference();
    Scope::announcing(state, ME, old)
        .amount(&Amount::Tally {
            event,
            who: Ref::EachPlayer,
            filter: Selector(filter),
        })
        .unwrap()
}

#[rstest]
#[case::any(vec![], 1)]
#[case::mine(vec![Pred::ControlledBy(Ref::You)], 1)]
#[case::theirs(vec![Pred::ControlledBy(Ref::Opponents)], 0)]
#[case::creature(vec![Pred::Type(CardType::Creature)], 1)]
#[case::land(vec![Pred::Type(CardType::Land)], 0)]
fn a_permanent_that_entered_counts_even_after_it_left(
    #[case] filter: Vec<Pred>,
    #[case] expected: i64,
) {
    let mut game = table();
    assert_eq!(tally(&game, TallyEvent::Entered, vec![]), 0);
    let enter = moved(&game, "Newcomer", Zone::Hand(ME), Zone::Battlefield);
    fold(&mut game, &[enter]);
    assert_eq!(tally(&game, TallyEvent::Entered, filter.clone()), expected);
    let leave = moved(&game, "Newcomer", Zone::Battlefield, Zone::Hand(ME));
    fold(&mut game, &[leave]);
    assert_eq!(tally(&game, TallyEvent::Entered, filter), expected);
}

#[rstest]
#[case::any(vec![], 2)]
#[case::owned_by_me(vec![Pred::OwnedBy(Ref::You)], 1)]
#[case::controlled_by_opponents(vec![Pred::ControlledBy(Ref::Opponents)], 1)]
fn a_permanent_put_into_a_graveyard_from_the_battlefield_died(
    #[case] filter: Vec<Pred>,
    #[case] expected: i64,
) {
    let mut game = table();
    let events = [
        moved(&game, "Old", Zone::Battlefield, Zone::Graveyard(ME)),
        moved(
            &game,
            "Guest",
            Zone::Battlefield,
            Zone::Graveyard(PlayerId(1)),
        ),
    ];
    fold(&mut game, &events);
    assert_eq!(tally(&game, TallyEvent::Died, filter), expected);
    assert_eq!(tally(&game, TallyEvent::Entered, vec![]), 0);
}

#[rstest]
fn a_move_that_is_no_death_is_not_one_and_the_next_turn_clears_the_record() {
    let mut game = table();
    let events = [
        moved(&game, "Old", Zone::Battlefield, Zone::Hand(ME)),
        moved(&game, "Newcomer", Zone::Hand(ME), Zone::Graveyard(ME)),
    ];
    fold(&mut game, &events);
    assert_eq!(tally(&game, TallyEvent::Died, vec![]), 0);
    let dies = moved(
        &game,
        "Guest",
        Zone::Battlefield,
        Zone::Graveyard(PlayerId(1)),
    );
    fold(&mut game, &[dies]);
    assert_eq!(tally(&game, TallyEvent::Died, vec![]), 1);
    let turn = Event::TurnBegan {
        number: 2,
        active: PlayerId(1),
        extra: false,
    };
    fold(&mut game, &[turn]);
    assert_eq!(tally(&game, TallyEvent::Died, vec![]), 0);
}
