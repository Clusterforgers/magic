//! Looking at libraries: what the `LibraryLooked`, `CardsFound`, `Scried` and `Revealed` events say.
//!
//! They tell what a player learned and change no state; the effects that produce them (scry, search, reveal) are
//! task (f). The first three are private to the player who looked, so the protocol shows other seats counts only.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::zones::Zone;

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

/// A player scried: some of the cards looked at stay on top, the others go to the bottom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Scried {
    /// Who scried.
    pub player: PlayerId,
    /// The cards that stay on top, first to last.
    pub top: Vec<ObjectId>,
    /// The cards put on the bottom.
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
