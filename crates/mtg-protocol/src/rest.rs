//! Bodies of the account endpoints of `mtg-server` (`/api/auth/*`), with TypeScript bindings.
//!
//! These are the REST types the web takes from the generated bindings. The game bodies
//! (`POST /api/games`, `GET /api/games/{id}`) are still written by hand on the web side.

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

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

/// The signed-in account, as `register`, `login` and `GET /api/auth/me` answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UserView {
    /// The account's id (a UUID).
    pub id: String,
    /// The username as it was registered.
    pub username: String,
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
    /// 401: the username and password do not match an account; the same answer for an unknown
    /// username and a wrong password.
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
    pub const ALL: [AuthErrorCode; 14] = [
        AuthErrorCode::InvalidRequest,
        AuthErrorCode::InvalidUsername,
        AuthErrorCode::InvalidPassword,
        AuthErrorCode::UsernameTaken,
        AuthErrorCode::RegistrationClosed,
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
