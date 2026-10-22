//! Bodies of the game browser's endpoints of `mtg-server`: the public listing (`GET /api/lobbies`),
//! joining by a game's id or its join code, the look-up of one game (`GET /api/games/{id}`), the
//! host's visibility switch (`PATCH /api/games/{id}`) and the error codes of all of them, with
//! TypeScript bindings.

use std::fmt;

use mtg_cards::deck::Issue;
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Who may find and join a game.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Listed in the game browser while it is in its lobby; its join code works too.
    Public,
    /// Reached only through its seat links and its join code; never listed.
    #[default]
    Invite,
}

impl Visibility {
    /// Every visibility, in declaration order.
    pub const ALL: [Visibility; 2] = [Visibility::Public, Visibility::Invite];

    /// The text on the wire and in the database.
    pub const fn as_str(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Invite => "invite",
        }
    }
}

/// One public game in its lobby, as `GET /api/lobbies` lists it. It holds no seat token, join code
/// or seat link: the id is all a visitor needs to ask to join.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LobbyGame {
    /// The game's id; `POST /api/games/{id}/join` takes it.
    pub game: String,
    /// What the host, who holds seat 0, is called.
    pub host: String,
    /// The format key the game is played under: `normal` or `commander`.
    pub format: String,
    /// What the host set in place of the format's values; left out when nothing is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub house_rules: Option<HouseRules>,
    /// Seats somebody holds, the host's included.
    pub filled: u32,
    /// Seats in all; a lobby with `filled == total` is full and waits for everybody to be ready.
    pub total: u32,
    /// When the game was created, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub created_at: String,
    /// Whole seconds since the game was created, at the moment of the answer.
    pub age_secs: u64,
}

/// The answer of `GET /api/lobbies`: the public games still in their lobby, newest first, at most 50.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LobbyList {
    /// The games.
    pub games: Vec<LobbyGame>,
}

/// One seat of a game, as `GET /api/games/{id}` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct GameSeat {
    /// The seat number.
    pub seat: PlayerId,
    /// What the table calls the seat; empty while nobody holds it.
    pub name: String,
}

/// The answer of `GET /api/games/{id}`: what anybody who knows a game's id may learn. It holds no
/// seat token, join code or link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct GameInfo {
    /// The format key the game is played under: `normal` or `commander`.
    pub format: String,
    /// Whether every seat was ready and the game began.
    pub started: bool,
    /// Every seat, in seat order.
    pub seats: Vec<GameSeat>,
    /// The seats with a live connection.
    pub connected: Vec<PlayerId>,
    /// The seats that pressed Ready and did not take it back; every seat once the game began.
    pub ready: Vec<PlayerId>,
    /// How the game ended; null until it has.
    pub result: Option<GameResult>,
    /// Who may find the game; the host changes it with `PATCH /api/games/{id}` before the start.
    pub visibility: Visibility,
}

/// The body of `POST /api/games/{id}/join`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct JoinRequest {
    /// The name the table sees, 1 to 32 characters. A guest needs one; a signed-in visitor is named
    /// after the account when it is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    /// The deck the seat plays: the id of a built-in deck, `precon:<code>` (`GET /api/precons`) or the id of one of the signed-in visitor's stored
    /// decks. Absent, the seat keeps the deck it was made with, if any, and its holder chooses one with
    /// `PUT /api/games/{id}/seats/{n}/deck`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub deck: Option<String>,
}

/// The body of `POST /api/join`: a join code and, like [`JoinRequest`], a name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct JoinCodeRequest {
    /// The code as typed: eight symbols, case, spaces and hyphens do not matter.
    pub code: String,
    /// The name the table sees; see [`JoinRequest::name`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    /// The deck the seat plays; see [`JoinRequest::deck`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub deck: Option<String>,
}

/// The seat a join gives the visitor, and the game it is in.
///
/// `Debug` never prints the token.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Joined {
    /// The id of the game the seat is in. A join by code is the one way to learn it from the code,
    /// and only a right code answers it.
    pub game: String,
    /// The seat.
    pub seat: PlayerId,
    /// The seat's token, shown here and never again. Null when the signed-in visitor already held
    /// a seat of the game: it is handed that seat back and opens it through its session.
    pub token: Option<String>,
}

impl fmt::Debug for Joined {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Joined")
            .field("game", &self.game)
            .field("seat", &self.seat)
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// The body of `PATCH /api/games/{id}` and its answer: who may find the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct VisibilityBody {
    /// The visibility to set, or the one now in force.
    pub visibility: Visibility,
}

/// The body of `PUT /api/games/{id}/seats/{n}/deck`: the deck the seat plays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ChooseDeck {
    /// The id of a built-in deck (`GET /api/demo-decks`), `precon:<code>` (`GET /api/precons`) or the id of one of the signed-in holder's stored
    /// decks. A stored deck of another account is the same `unknown_deck` as one that does not exist.
    pub deck: String,
}

/// The `error` code of a request that names a deck for a seat (the creation of a game, a join and
/// `PUT .../deck`); the other failures of those requests keep the codes of [`LobbyErrorCode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DeckChoiceErrorCode {
    /// 404 (400 when a game is created): the id is no built-in deck and no deck of the caller's.
    UnknownDeck,
    /// 422: the deck check refuses the deck; the body also lists the `issues`.
    DeckBlocked,
    /// 409: the seat pressed Ready; it takes that back before it changes its deck.
    SeatReady,
}

impl DeckChoiceErrorCode {
    /// Every code, in declaration order.
    pub const ALL: [DeckChoiceErrorCode; 3] = [
        DeckChoiceErrorCode::UnknownDeck,
        DeckChoiceErrorCode::DeckBlocked,
        DeckChoiceErrorCode::SeatReady,
    ];

    /// The text on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            DeckChoiceErrorCode::UnknownDeck => "unknown_deck",
            DeckChoiceErrorCode::DeckBlocked => "deck_blocked",
            DeckChoiceErrorCode::SeatReady => "seat_ready",
        }
    }
}

/// The body of a refused deck (422 `deck_blocked`): why the deck check will not let it be played in
/// this game. The issues name cards, and go only to the account whose deck it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DeckBlockedBody {
    /// Always `deck_blocked`.
    pub error: DeckChoiceErrorCode,
    /// The refusal in plain words.
    pub message: String,
    /// What breaks the rules of the deck's format or of the game; empty when the deck is blocked
    /// only because it holds cards the game cannot play.
    pub issues: Vec<Issue>,
}

/// The `error` code of a failed game browser request; the body is `{error, message}`.
///
/// `message` is for people and may change, the code is for programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LobbyErrorCode {
    /// 400: the body, the seat number or the visibility is not what the endpoint takes.
    InvalidRequest,
    /// 400: a guest's name is empty, longer than 32 characters or has control characters.
    InvalidName,
    /// 401: no seat token and no valid session cookie.
    Unauthenticated,
    /// 403: the request comes from another site.
    ForbiddenOrigin,
    /// 403: only the host may do this.
    Forbidden,
    /// 404: no such game or code; an invite game reached by its id, a game or seat the caller
    /// holds no credential for and a started invite game look the same.
    NotFound,
    /// 409: every seat of the game is taken.
    Full,
    /// 409: the game has started, so its seats and visibility no longer change and the host can no
    /// longer close it.
    Started,
    /// 409: the seat was made with the game, not taken by a join, so it cannot be given back.
    NotClaimed,
    /// 413: the body is too large.
    PayloadTooLarge,
    /// 415: the body is not `application/json`.
    UnsupportedMediaType,
    /// 429: too many wrong join codes from one address; `Retry-After` says when to try again.
    TooManyAttempts,
    /// 429: too many requests for the listing from one address; `Retry-After` says when to try again.
    RateLimited,
    /// 503: the server runs without a database and the request needs an account.
    AccountsUnavailable,
    /// 503: the server cannot list its games right now.
    LobbiesUnavailable,
    /// 503: the database or the server is not available right now.
    Unavailable,
    /// 500: the server failed; the details stay in its log.
    Internal,
}

impl LobbyErrorCode {
    /// Every code, in declaration order.
    pub const ALL: [LobbyErrorCode; 17] = [
        LobbyErrorCode::InvalidRequest,
        LobbyErrorCode::InvalidName,
        LobbyErrorCode::Unauthenticated,
        LobbyErrorCode::ForbiddenOrigin,
        LobbyErrorCode::Forbidden,
        LobbyErrorCode::NotFound,
        LobbyErrorCode::Full,
        LobbyErrorCode::Started,
        LobbyErrorCode::NotClaimed,
        LobbyErrorCode::PayloadTooLarge,
        LobbyErrorCode::UnsupportedMediaType,
        LobbyErrorCode::TooManyAttempts,
        LobbyErrorCode::RateLimited,
        LobbyErrorCode::AccountsUnavailable,
        LobbyErrorCode::LobbiesUnavailable,
        LobbyErrorCode::Unavailable,
        LobbyErrorCode::Internal,
    ];

    /// The text on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            LobbyErrorCode::InvalidRequest => "invalid_request",
            LobbyErrorCode::InvalidName => "invalid_name",
            LobbyErrorCode::Unauthenticated => "unauthenticated",
            LobbyErrorCode::ForbiddenOrigin => "forbidden_origin",
            LobbyErrorCode::Forbidden => "forbidden",
            LobbyErrorCode::NotFound => "not_found",
            LobbyErrorCode::Full => "full",
            LobbyErrorCode::Started => "started",
            LobbyErrorCode::NotClaimed => "not_claimed",
            LobbyErrorCode::PayloadTooLarge => "payload_too_large",
            LobbyErrorCode::UnsupportedMediaType => "unsupported_media_type",
            LobbyErrorCode::TooManyAttempts => "too_many_attempts",
            LobbyErrorCode::RateLimited => "rate_limited",
            LobbyErrorCode::AccountsUnavailable => "accounts_unavailable",
            LobbyErrorCode::LobbiesUnavailable => "lobbies_unavailable",
            LobbyErrorCode::Unavailable => "unavailable",
            LobbyErrorCode::Internal => "internal",
        }
    }
}
