//! The manual actions on the seat's own library: looking at its top cards, putting the cards it looked at back in
//! any order, on the bottom or into the graveyard, and shuffling it, each as the events a spell that does the same
//! produces.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::events::{Event, ZoneCause};
use crate::library::{LibraryLooked, Scried, Surveilled, top_cards};
use crate::manual::{MAX_CARDS, ManualRefusal};
use crate::mulligan::shuffled;
use crate::state::GameState;
use crate::zones::Zone;

/// The seat looks at the top `count` cards of its library, all of a shorter one; the cards are known to it alone
/// until they leave the library or it is shuffled.
pub(crate) fn look_top(
    state: &GameState,
    player: PlayerId,
    count: u32,
) -> Result<Vec<Event>, ManualRefusal> {
    if !(1..=MAX_CARDS).contains(&count) {
        return Err(ManualRefusal::OutOfRange("the number of cards"));
    }
    let cards = top_cards(state, player, usize::try_from(count).unwrap_or(0));
    if cards.is_empty() {
        return Err(ManualRefusal::NotEnough("cards in the library"));
    }
    Ok(vec![Event::LibraryLooked(LibraryLooked { player, cards })])
}

/// The seat puts the top cards of its library, which it looked at, back: `top` on top as listed from the top down,
/// `bottom` under the library as listed from the top down, `graveyard` into its graveyard. A list with no bottom is a
/// surveil; any other is a scry of the cards that stay in the library, the cards for the graveyard just under the top
/// ones until their moves follow.
pub(crate) fn arrange_top(
    state: &GameState,
    player: PlayerId,
    top: &[ObjectId],
    bottom: &[ObjectId],
    graveyard: &[ObjectId],
) -> Result<Vec<Event>, ManualRefusal> {
    let named: Vec<ObjectId> = top.iter().chain(bottom).chain(graveyard).copied().collect();
    if named.is_empty() || named.len() > usize::try_from(MAX_CARDS).unwrap_or(usize::MAX) {
        return Err(ManualRefusal::OutOfRange("the number of cards"));
    }
    let on_top = top_cards(state, player, named.len());
    let distinct = named
        .iter()
        .enumerate()
        .all(|(at, card)| !named[..at].contains(card));
    if !distinct || on_top.len() < named.len() || !named.iter().all(|card| on_top.contains(card)) {
        return Err(ManualRefusal::NotLookedAt);
    }
    if !on_top.iter().all(|card| state.is_shown_to(player, *card)) {
        return Err(ManualRefusal::NotLookedAt);
    }
    let library = Zone::Library(player);
    let moves = graveyard.iter().map(|object| Event::ZoneChanged {
        object: *object,
        from: library,
        to: Zone::Graveyard(player),
        cause: ZoneCause::Moved,
    });
    let arranged = if bottom.is_empty() && !graveyard.is_empty() {
        Event::Surveilled(Surveilled {
            player,
            top: top.to_vec(),
            graveyard: graveyard.to_vec(),
        })
    } else {
        Event::Scried(Scried {
            player,
            top: top.iter().chain(graveyard).copied().collect(),
            bottom: bottom.to_vec(),
        })
    };
    Ok(std::iter::once(arranged).chain(moves).collect())
}

/// The seat shuffles its library into an order from the game's random stream, logged whole.
pub(crate) fn shuffle(state: &GameState, player: PlayerId) -> Vec<Event> {
    vec![Event::LibraryShuffled {
        player,
        order: shuffled(state, 0, state.zone(Zone::Library(player)).to_vec()),
    }]
}
