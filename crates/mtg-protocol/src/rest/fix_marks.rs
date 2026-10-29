//! Bodies of the fix-mark endpoints of `mtg-server`: `PUT /api/fix-marks/{oracle_id}`,
//! `DELETE /api/fix-marks/{oracle_id}` and `GET /api/fix-marks`. A mark says a card is important to
//! fix; the errors use `DeckErrorCode`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The longest note of a mark, in characters.
pub const MAX_FIX_NOTE_CHARS: usize = 280;

/// The body of `PUT /api/fix-marks/{oracle_id}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct FixMarkBody {
    /// What is wrong with the card, up to `MAX_FIX_NOTE_CHARS` characters without control
    /// characters other than line breaks; an empty note is none.
    #[serde(default)]
    pub note: Option<String>,
}

/// A card the account marked as important to fix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct FixMarkView {
    /// The marked card's oracle id.
    pub oracle_id: String,
    /// The card's name.
    pub name: String,
    /// The note of the mark, if it has one.
    pub note: Option<String>,
    /// When the card was marked, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub created_at: String,
}

/// The answer of `GET /api/fix-marks`: the account's marks, the newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct FixMarkList {
    /// The marks.
    pub marks: Vec<FixMarkView>,
}
