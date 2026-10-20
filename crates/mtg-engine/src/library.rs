//! Looking at libraries: what the `LibraryLooked`, `CardsFound`, `Scried` and `Revealed` events say.
//!
//! They tell what a player learned; only `Scried` also changes the game, by putting the cards looked at back
//! (`fold/zones.rs`). The first three are private to the player who looked, so the protocol shows other seats counts
//! only; a reveal is public.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::state::GameState;
use crate::zones::Zone;

/// The top `count` cards of a player's library, the top card first; fewer when the library is shorter.
pub(crate) fn top_cards(state: &GameState, player: PlayerId, count: usize) -> Vec<ObjectId> {
    state
        .zone(Zone::Library(player))
        .iter()
        .rev()
        .take(count)
        .copied()
        .collect()
}

/// A player looked at cards in a library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LibraryLooked {
    /// Who looked.
    pub player: PlayerId,
    /// The cards, top first.
    pub cards: Vec<ObjectId>,
}

/// A player found cards by searching a zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardsFound {
    /// Who searched.
    pub player: PlayerId,
    /// The zone searched.
    pub zone: Zone,
    /// The cards found.
    pub cards: Vec<ObjectId>,
}

/// A player scried: some of the cards looked at stay on top, the others go to the bottom (CR 701.22).
///
/// Together the two lists are the top cards of the library, so the event reorders it. Both read from the top down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Scried {
    /// Who scried.
    pub player: PlayerId,
    /// The cards that stay on top, the next card drawn first.
    pub top: Vec<ObjectId>,
    /// The cards put on the bottom, the last of them the bottom card.
    pub bottom: Vec<ObjectId>,
}

/// A player revealed cards to every player.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Revealed {
    /// Who revealed them.
    pub player: PlayerId,
    /// The cards.
    pub cards: Vec<ObjectId>,
}
