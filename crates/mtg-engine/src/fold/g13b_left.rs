//! Goal: a creature that leaves the battlefield for any zone counts once for the player who controlled it
//! (`Tally::CreaturesLeftUnder`); a noncreature permanent, a card leaving another zone and the other seats count none,
//! at tables of two, three and four seats.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::apply_event;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::state::{GameState, Tally};
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

fn table(seats: usize) -> GameState {
    let hand = |seat: usize| {
        if seat == 0 {
            vec![creature("Rat", 1, 1)]
        } else {
            Vec::new()
        }
    };
    let field = |seat: usize| {
        let mut cards = vec![creature(&format!("Guest {seat}"), 2, 2)];
        if seat == 0 {
            cards.push(creature("Bear", 2, 2));
            cards.push(Arc::new(
                CardDefinition::new("Relic").with_type(CardType::Artifact),
            ));
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

fn leave(game: &mut GameState, name: &str, from: Zone, to: Zone) {
    let event = Event::ZoneChanged {
        object: id(game, name),
        from,
        to,
        cause: ZoneCause::Moved,
    };
    apply_event(game, &event).unwrap();
}

#[rstest]
fn a_creature_leaving_counts_for_its_controller_wherever_it_goes(
    #[values(2, 3, 4)] seats: usize,
    #[values(Zone::Hand(ME), Zone::Exile, Zone::Graveyard(ME))] to: Zone,
) {
    let mut game = table(seats);
    leave(&mut game, "Rat", Zone::Hand(ME), to);
    leave(&mut game, "Relic", Zone::Battlefield, to);
    assert_eq!(game.tallied(ME, Tally::CreaturesLeftUnder), 0);
    leave(&mut game, "Bear", Zone::Battlefield, to);
    assert_eq!(game.tallied(ME, Tally::CreaturesLeftUnder), 1);
    for seat in 1..seats {
        let player = PlayerId(u64::try_from(seat).unwrap());
        assert_eq!(game.tallied(player, Tally::CreaturesLeftUnder), 0);
    }
}
