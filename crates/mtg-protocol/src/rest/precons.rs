//! Bodies of the precon endpoints of `mtg-server`: `GET /api/precons` and `GET /api/precons/{code}`,
//! and the error codes of both.

use mtg_cards::deck::{DeckState, DeckZone, Issue, Summary};
use mtg_cards::support::{Reason, Support};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::cards::CardHit;

/// The prefix that names a precon where a request names a deck: `precon:<code>` in the `deck` of a
/// seat, a join or the creation of a game.
pub const PRECON_DECK_PREFIX: &str = "precon:";

/// A precon in the list of `GET /api/precons`, with its deck check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PreconListItem {
    /// The precon's code: lower case letters, digits and hyphens, unique among the stored precons
    /// (the set code and the name's slug). It names the precon in the paths of the API and, behind
    /// `precon:`, as a deck.
    pub code: String,
    /// The deck's name.
    pub name: String,
    /// The code of the set or product the deck came with, as the source writes it (`C18`).
    pub set: String,
    /// The kind of deck, such as `Commander Deck`.
    pub deck_type: Option<String>,
    /// The release date as `YYYY-MM-DD`.
    pub release_date: Option<String>,
    /// The names of the cards in the command zone.
    pub commanders: Vec<String>,
    /// Copies in the commander and main boards: the cards a game plays.
    pub card_count: u32,
    /// The format the deck check judged it in: `commander` for a deck with a commander, else
    /// `freeform`.
    pub format: String,
    /// The deck check: `verified`, `read`, `manual_play` or `blocked`. It is derived from the cards' rules and
    /// the engine's capabilities each time they change, so it is never `unchecked`.
    pub state: DeckState,
    /// True exactly when `state` is `verified`.
    pub verified: bool,
    /// Whether a game of this server takes the deck today: the deck check does not block it and the
    /// rules of the game's format hold. A deck that is not playable is refused with `deck_blocked`
    /// when it is chosen; `GET /api/precons/{code}` lists why.
    pub playable: bool,
    /// The counts of the check: copies by support state, overall and per board.
    pub summary: Summary,
}

/// The answer of `GET /api/precons`: every stored precon, newest release first, then by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PreconList {
    /// The precons.
    pub precons: Vec<PreconListItem>,
}

/// One line of a precon: copies of a card and how far the game plays it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PreconCardView {
    /// The card's name as the deck spells it.
    pub name: String,
    /// How many copies.
    pub count: u32,
    /// The printing the deck names, if it names one.
    pub printing_id: Option<String>,
    /// The card as a list shows it, in the art of the deck's printing; null when the deck names a
    /// card the database does not hold.
    pub card: Option<CardHit>,
    /// How far the game plays the card; `unplayable` for a card the database does not hold.
    pub support: Support,
    /// The clauses left to the table, each with the span of oracle text it covers.
    pub reasons: Vec<Reason>,
}

/// The cards of one board of a precon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PreconBoard {
    /// The board: `commander`, `main` or `sideboard`.
    pub board: DeckZone,
    /// Copies on the board.
    pub copies: u32,
    /// The lines in the deck's order.
    pub cards: Vec<PreconCardView>,
}

/// The answer of `GET /api/precons/{code}`: the precon with its cards by board.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PreconView {
    /// The precon as the list shows it.
    pub precon: PreconListItem,
    /// What keeps the deck out of a game: the legality issues of the deck check and the rules of the
    /// game's format. Empty exactly when the deck is `playable`, except for a deck blocked only
    /// because it holds cards the game cannot play.
    pub issues: Vec<Issue>,
    /// The boards that hold cards, commander first, then main and sideboard.
    pub boards: Vec<PreconBoard>,
}

/// The `error` code of a failed precon request; the body is `{error, message}`.
///
/// `message` is for people and may change, the code is for programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PreconErrorCode {
    /// 404: no stored precon has that code; a path that is no code looks the same.
    NotFound,
    /// 429: too many requests from one address; `Retry-After` says when to try again.
    RateLimited,
    /// 503: the server runs without a database, so it has no precons.
    CardsUnavailable,
    /// 503: the database or the server is not available right now.
    Unavailable,
    /// 500: the server failed; the details stay in its log.
    Internal,
}

impl PreconErrorCode {
    /// Every code, in declaration order.
    pub const ALL: [PreconErrorCode; 5] = [
        PreconErrorCode::NotFound,
        PreconErrorCode::RateLimited,
        PreconErrorCode::CardsUnavailable,
        PreconErrorCode::Unavailable,
        PreconErrorCode::Internal,
    ];

    /// The text on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            PreconErrorCode::NotFound => "not_found",
            PreconErrorCode::RateLimited => "rate_limited",
            PreconErrorCode::CardsUnavailable => "cards_unavailable",
            PreconErrorCode::Unavailable => "unavailable",
            PreconErrorCode::Internal => "internal",
        }
    }
}
