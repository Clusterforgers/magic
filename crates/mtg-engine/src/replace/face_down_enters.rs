//! Goal: a card that enters with counters gets them on the permanent it becomes, also when the batch moves it before it
//! enters (a face-down pile exiles it first), and gets none when it is manifested or cloaked, since a face-down permanent
//! has no abilities (CR 708.2); at two to four seats.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::{Processed, process};
use crate::events::{Event, ZoneCause};
use crate::fixtures::state;
use crate::fold::apply_event;
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

const ENTERS_WITH_COUNTERS: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"-1/-1","count":2}}}}"#;

/// Seat 0 holds a creature card that enters with two -1/-1 counters.
fn game(seats: usize) -> GameState {
    let auntie = Arc::new(
        CardDefinition::new("Auntie")
            .with_type(CardType::Creature)
            .with_power_toughness(3, 3)
            .with_rules(serde_json::from_str::<Ability>(ENTERS_WITH_COUNTERS).unwrap()),
    );
    let mut hands = vec![vec![auntie]];
    hands.resize(seats, Vec::new());
    state(hands, vec![Vec::new(); seats])
}

/// Applies the batch the replacement step makes of `moves` for the card in the hand of seat 0, and the counters it
/// puts on the card: each event must apply in turn.
fn counters_after(game: &GameState, moves: &[(Zone, Zone, ZoneCause)]) -> Vec<i32> {
    let card = game.zone(Zone::Hand(ME))[0];
    let batch: Vec<Event> = moves
        .iter()
        .map(|(from, to, cause)| Event::ZoneChanged {
            object: card,
            from: *from,
            to: *to,
            cause: *cause,
        })
        .collect();
    let Processed::Events(events) = process(game, batch, &[]).unwrap() else {
        panic!("nobody is asked");
    };
    let mut after = game.clone();
    for event in &events {
        apply_event(&mut after, event).unwrap();
    }
    events
        .iter()
        .filter_map(|event| match event {
            Event::CountersChanged(changed) => Some(changed.delta),
            _ => None,
        })
        .collect()
}

#[rstest]
fn a_card_that_enters_face_up_gets_its_counters(#[values(2, 3, 4)] seats: usize) {
    let hand = Zone::Hand(ME);
    let moved = (hand, Zone::Battlefield, ZoneCause::Moved);
    assert_eq!(counters_after(&game(seats), &[moved]), vec![2]);
}

#[rstest]
fn the_counters_follow_the_card_through_an_earlier_move_of_the_batch(
    #[values(2, 3, 4)] seats: usize,
) {
    let hand = Zone::Hand(ME);
    let aside = (hand, Zone::Exile, ZoneCause::Moved);
    let back = (Zone::Exile, Zone::Battlefield, ZoneCause::Moved);
    assert_eq!(counters_after(&game(seats), &[aside, back]), vec![2]);
}

#[rstest]
fn a_manifested_or_cloaked_card_enters_without_the_counters_it_prints(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] cloaked: bool,
    #[values(true, false)] through_exile: bool,
) {
    let game = game(seats);
    let hand = Zone::Hand(ME);
    let source = game.object(game.zone(hand)[0]).unwrap().reference();
    let cause = if cloaked {
        ZoneCause::Cloaked { by: ME }
    } else {
        ZoneCause::Manifested { by: ME }
    };
    let mut moves = Vec::new();
    let from = if through_exile {
        moves.push((hand, Zone::Exile, ZoneCause::Sealed { by: source }));
        Zone::Exile
    } else {
        hand
    };
    moves.push((from, Zone::Battlefield, cause));
    assert_eq!(counters_after(&game, &moves), Vec::<i32>::new());
}
