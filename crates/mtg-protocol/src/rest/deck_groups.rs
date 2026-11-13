//! Bodies of the deck-group endpoints of `mtg-server` (`/api/deck-groups`, `PUT /api/decks/{id}/group`):
//! an account's named groups of its decks, in its order, each collapsed or open. A deck sits in at most
//! one group; the errors use `DeckErrorCode`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Most groups one account keeps.
pub const MAX_DECK_GROUPS_PER_USER: u32 = 50;
/// Most characters in a group's name.
pub const MAX_DECK_GROUP_NAME_CHARS: usize = 60;

/// One group of the account's decks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DeckGroupView {
    /// The group's id.
    pub id: String,
    /// The group's name.
    pub name: String,
    /// Whether the owner keeps the group collapsed.
    pub collapsed: bool,
    /// The ids of the decks in the group, the most recently saved first.
    pub decks: Vec<String>,
}

/// The answer of `GET /api/deck-groups`: the account's groups in its order; a deck in none is ungrouped.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DeckGroups {
    /// The groups, in the owner's order.
    pub groups: Vec<DeckGroupView>,
}

/// The body of `POST /api/deck-groups`: a new group, open and empty, after the others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct NewDeckGroup {
    /// The name, 1 to 60 characters once trimmed.
    pub name: String,
}

/// The body of `PATCH /api/deck-groups/{id}`: what changes; a missing field stays.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DeckGroupChange {
    /// The new name, 1 to 60 characters once trimmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    /// Whether the group is collapsed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub collapsed: Option<bool>,
}

/// The body of `PUT /api/deck-groups/order`: every group id of the account once, in the new order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DeckGroupOrder {
    /// The group ids.
    pub ids: Vec<String>,
}

/// The body of `PUT /api/decks/{id}/group`: the group the deck goes to, or `null` for none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DeckGroupChoice {
    /// The group's id; `null` ungroups the deck.
    pub group: Option<String>,
}
