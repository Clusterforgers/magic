//! Body of the account preference endpoints of `mtg-server`: `GET /api/me/prefs` and `PUT /api/me/prefs`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::message::{AutoChoices, Stop};

/// The most default stops an account keeps.
pub const MAX_DEFAULT_STOPS: usize = 40;

/// The pace preferences of an account, kept on the server and applied to every game it plays.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UserPrefs {
    /// The lasting stops a new game starts with.
    pub default_stops: Vec<Stop>,
    /// Whether the account's seats end their own turns by hand.
    pub end_turn_by_hand: bool,
    /// The decisions the server answers for the account's seats; off when absent.
    #[serde(default)]
    pub auto_choices: AutoChoices,
}
