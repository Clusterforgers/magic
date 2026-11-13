//! Bodies of the admin's game endpoints of `mtg-server`: the live games (`GET /api/admin/games`) and ending one
//! (`POST /api/admin/games/{id}/end`), with TypeScript bindings.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Most characters in the reason an admin gives for ending a game.
pub const MAX_END_REASON_CHARS: usize = 200;

/// One seat of a live game, as the admin sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AdminGameSeat {
    /// What the table calls the seat; empty while it is open.
    pub name: String,
    /// Whether a client is connected to it now.
    pub connected: bool,
}

/// One live game or lobby of `GET /api/admin/games`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AdminGame {
    /// The game's id.
    pub game: String,
    /// The format key: `normal` or `commander`.
    pub format: String,
    /// `public` or `invite`.
    pub visibility: String,
    /// Whether the game runs; false while it is in its lobby.
    pub started: bool,
    /// Whether the game is a tutorial table, which is never recorded.
    pub tutorial: bool,
    /// Every seat in seat order.
    pub seats: Vec<AdminGameSeat>,
    /// When the game was created, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub created_at: String,
    /// When a seat last acted or was last seen connected, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub last_activity: String,
}

/// The answer of `GET /api/admin/games`: every lobby and running game, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AdminGameList {
    /// The games.
    pub games: Vec<AdminGame>,
}

/// The body of `POST /api/admin/games/{id}/end`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct EndGameBody {
    /// Why, for the players; blank counts as none, at most [`MAX_END_REASON_CHARS`] characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reason: Option<String>,
}
