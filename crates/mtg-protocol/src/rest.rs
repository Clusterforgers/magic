//! Bodies of the account endpoints of `mtg-server` (`/api/auth/*`), of the signed-in user's game
//! list (`/api/me/games`), of the card endpoints (`/api/cards`), of the deck endpoints
//! (`/api/decks`), of the fix-mark endpoints (`/api/fix-marks`), of the precon endpoints
//! (`/api/precons`) and of the game browser (`/api/lobbies`, joining), with TypeScript bindings.
//!
//! These are the REST types the web takes from the generated bindings. The body of
//! `POST /api/games` is still written by hand on both sides.

mod cards;
mod decks;
mod fix_marks;
mod lobbies;
mod precons;

use std::fmt;

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use cards::{
    CardColor, CardDetail, CardFaceView, CardHit, CardImages, CardSearchPage, DEFAULT_PER_PAGE,
    ImageLinks, MAX_PAGE, MAX_PER_PAGE, PrintingView, RulingView,
};
pub use decks::{
    CardSuggestion, CheckRequest, DeckBody, DeckErrorCode, DeckLineBody, DeckLineView, DeckList,
    DeckListItem, DeckView, IgnoredCards, ImportRequest, ImportResult, ImportSource, ListProblem,
    MAX_DECK_CARDS, MAX_DECK_LINES, MAX_DECK_NAME_CHARS, MAX_DECK_NOTES_CHARS, MAX_DECKS_PER_USER,
    MAX_IMPORT_CHARS, MAX_IMPORT_LINES, TableRules, UnresolvedLine, UnresolvedReason,
};
pub use fix_marks::{FixMarkBody, FixMarkList, FixMarkView, MAX_FIX_NOTE_CHARS};
pub use lobbies::{
    ChooseDeck, DeckBlockedBody, DeckChoiceErrorCode, GameInfo, GameSeat, JoinCodeRequest,
    JoinRequest, Joined, LobbyErrorCode, LobbyGame, LobbyList, Visibility, VisibilityBody,
};
pub use precons::{
    PRECON_DECK_PREFIX, PreconBoard, PreconCardView, PreconErrorCode, PreconList, PreconListItem,
    PreconView,
};

/// Fewest characters in a username.
pub const USERNAME_MIN_CHARS: usize = 3;
/// Most characters in a username.
pub const USERNAME_MAX_CHARS: usize = 32;
/// Fewest characters in a password.
pub const PASSWORD_MIN_CHARS: usize = 10;
/// Most characters in a password.
pub const PASSWORD_MAX_CHARS: usize = 128;

/// A username and a password: the body of `POST /api/auth/register` and `POST /api/auth/login`.
///
/// `Debug` never prints the password.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Credentials {
    /// 3 to 32 ASCII letters, digits, `_` or `-`; unique ignoring case, shown as typed.
    pub username: String,
    /// 10 to 128 characters, compared exactly as sent.
    pub password: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// The body of `POST /api/auth/password`: the password in force and the one to replace it.
///
/// `Debug` never prints either password.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PasswordChange {
    /// The account's password now, compared exactly as sent.
    pub current_password: String,
    /// 10 to 128 characters, like any password.
    pub new_password: String,
}

impl fmt::Debug for PasswordChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasswordChange")
            .field("current_password", &"<redacted>")
            .field("new_password", &"<redacted>")
            .finish()
    }
}

/// The body of `DELETE /api/auth/account`: the password that confirms the deletion.
///
/// `Debug` never prints the password.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AccountDeletion {
    /// The account's password, compared exactly as sent.
    pub password: String,
}

impl fmt::Debug for AccountDeletion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountDeletion")
            .field("password", &"<redacted>")
            .finish()
    }
}

/// The signed-in account, as `register`, `login` and `GET /api/auth/me` answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UserView {
    /// The account's id (a UUID).
    pub id: String,
    /// The username as it was registered.
    pub username: String,
    /// Whether this is the admin account, the only one that makes accounts; read from the database on every
    /// request, so the client may show an Accounts page on it but the server stays the judge.
    pub is_admin: bool,
}

/// The account `POST /api/admin/users` made; it is not signed in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CreatedAccount {
    /// The new account's id (a UUID).
    pub id: String,
    /// The username as it was registered.
    pub username: String,
}

/// One row of `GET /api/admin/users`; never a password hash or a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AccountRow {
    /// The account's id (a UUID).
    pub id: String,
    /// The username as it was registered.
    pub username: String,
    /// When the account was made, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub created_at: String,
    /// Whether this is the admin account.
    pub is_admin: bool,
}

/// The answer of `GET /api/admin/users`: every account, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AccountList {
    /// The accounts.
    pub users: Vec<AccountRow>,
}

/// The answer of `GET /api/auth/options`, which anybody may ask: what a visitor can do before signing in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AccountOptions {
    /// Whether `POST /api/auth/register` accepts new accounts (`MTG_REGISTRATION=open`).
    pub registration_open: bool,
}

/// The `error` code of a failed account request; the body is `{error, message}`.
///
/// `message` is for people and may change, the code is for programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AuthErrorCode {
    /// 400: the body is not a `Credentials` object.
    InvalidRequest,
    /// 400: the username breaks the rules above.
    InvalidUsername,
    /// 400: the password breaks the rules above.
    InvalidPassword,
    /// 409: the username is taken (ignoring case).
    UsernameTaken,
    /// 403: the server does not accept new accounts.
    RegistrationClosed,
    /// 403: the signed-in account is not the admin, which the route needs.
    Forbidden,
    /// 401: the username and password do not match an account; the same answer for an unknown
    /// username and a wrong password. Also the answer to a wrong password that confirms a
    /// password change or an account deletion.
    InvalidCredentials,
    /// 429: too many failed attempts; `Retry-After` says when to try again.
    TooManyAttempts,
    /// 401: no valid session cookie.
    Unauthenticated,
    /// 403: the request comes from another site.
    ForbiddenOrigin,
    /// 413: the body is too large.
    PayloadTooLarge,
    /// 415: the body is not `application/json`.
    UnsupportedMediaType,
    /// 503: the server runs without a database, so it has no accounts.
    AccountsUnavailable,
    /// 503: the database or the server is not available right now.
    Unavailable,
    /// 500: the server failed; the details stay in its log.
    Internal,
}

impl AuthErrorCode {
    /// Every code, in declaration order.
    pub const ALL: [AuthErrorCode; 15] = [
        AuthErrorCode::InvalidRequest,
        AuthErrorCode::InvalidUsername,
        AuthErrorCode::InvalidPassword,
        AuthErrorCode::UsernameTaken,
        AuthErrorCode::RegistrationClosed,
        AuthErrorCode::Forbidden,
        AuthErrorCode::InvalidCredentials,
        AuthErrorCode::TooManyAttempts,
        AuthErrorCode::Unauthenticated,
        AuthErrorCode::ForbiddenOrigin,
        AuthErrorCode::PayloadTooLarge,
        AuthErrorCode::UnsupportedMediaType,
        AuthErrorCode::AccountsUnavailable,
        AuthErrorCode::Unavailable,
        AuthErrorCode::Internal,
    ];

    /// The text on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            AuthErrorCode::InvalidRequest => "invalid_request",
            AuthErrorCode::InvalidUsername => "invalid_username",
            AuthErrorCode::InvalidPassword => "invalid_password",
            AuthErrorCode::UsernameTaken => "username_taken",
            AuthErrorCode::RegistrationClosed => "registration_closed",
            AuthErrorCode::Forbidden => "forbidden",
            AuthErrorCode::InvalidCredentials => "invalid_credentials",
            AuthErrorCode::TooManyAttempts => "too_many_attempts",
            AuthErrorCode::Unauthenticated => "unauthenticated",
            AuthErrorCode::ForbiddenOrigin => "forbidden_origin",
            AuthErrorCode::PayloadTooLarge => "payload_too_large",
            AuthErrorCode::UnsupportedMediaType => "unsupported_media_type",
            AuthErrorCode::AccountsUnavailable => "accounts_unavailable",
            AuthErrorCode::Unavailable => "unavailable",
            AuthErrorCode::Internal => "internal",
        }
    }
}

/// Where a game in the user's list stands. Games the server dropped are not listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MyGameStatus {
    /// Created; not every seat has pressed ready yet.
    Lobby,
    /// Being played.
    Running,
    /// Over; `result` says how it ended.
    Finished,
    /// The server could not bring it back after a restart; `reason` says why and the user can
    /// remove it from the list.
    Failed,
}

/// One seat of a listed game, as the user may know it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MyGameSeat {
    /// The seat number.
    pub seat: PlayerId,
    /// The name the seat was given when the game was made.
    pub name: String,
    /// The username of the account sitting there; null for a guest seat.
    pub user: Option<String>,
}

/// One game in the signed-in user's list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MyGame {
    /// The game's id; `GET /api/games/{id}` and the socket `/ws/{id}` take it.
    pub game: String,
    /// The user's own seat.
    pub seat: PlayerId,
    /// The user's own seat's name.
    pub name: String,
    /// Where the game stands.
    pub status: MyGameStatus,
    /// When the game was created, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub created_at: String,
    /// The other seats in seat order, each with its name and, when an account sits there, the
    /// username.
    pub others: Vec<MyGameSeat>,
    /// How the game ended; null until it has.
    pub result: Option<GameResult>,
    /// Why a failed game could not be restored, in words for a player; null for every other status.
    pub reason: Option<String>,
}

/// The answer of `GET /api/me/games`: the user's games, newest first, at most 100.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MyGames {
    /// The games.
    pub games: Vec<MyGame>,
}
