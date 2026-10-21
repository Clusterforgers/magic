//! The account endpoints: `POST /api/auth/register`, `login`, `logout` and `password`, `DELETE
//! /api/auth/account`, and `GET /api/auth/me`.
//!
//! A session is a random 256-bit token in the `mtg_session` cookie; the database keeps only its
//! SHA-256. Passwords are Argon2id hashes. Logging in answers the same way for a username that
//! does not exist and for a wrong password, and spends the same time on both. Failed logins and
//! registrations are counted per address and per username ([`Throttle`]).
//!
//! Accounts live in Postgres, so a server started without a database answers every endpoint here
//! with 503 `accounts_unavailable` and its games stay guest-only.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::header::{RETRY_AFTER, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use mtg_protocol::rest::{AuthErrorCode, Credentials, UserView};
use sqlx::PgPool;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::accounts::{AccountError, Accounts, User};
use crate::api::{ApiError, read_json};
use crate::config::{AuthConfig, Registration};
use crate::cookie;
use crate::credentials::{Username, check_password, password_rules};
use crate::passwords::Passwords;
use crate::secret::{SessionToken, TokenDigest};
use crate::server::AppState;
use crate::throttle::{Blocked, Throttle};

mod settings;

pub(crate) use settings::{change_password, delete_account};

/// What the account endpoints share: the tables, the hasher and the counters of failed tries.
#[derive(Debug)]
pub(crate) struct Auth {
    /// `None` on a server without a database.
    accounts: Option<Accounts>,
    passwords: Passwords,
    throttle: Throttle,
    config: AuthConfig,
}

impl Auth {
    /// The account service for `config`, over `pool` when the server has a database.
    pub(crate) fn new(config: &AuthConfig, pool: Option<PgPool>) -> Auth {
        Auth {
            accounts: pool.map(Accounts::new),
            passwords: Passwords::new(&config.hash_cost, config.hash_jobs),
            throttle: Throttle::new(config.limits),
            config: config.clone(),
        }
    }

    /// Starts the background work: the stand-in password hash is made first, then expired
    /// sessions are deleted every `purge_every` until `stop` fires. `None` without a database.
    pub(crate) fn spawn_upkeep(&self, mut stop: watch::Receiver<bool>) -> Option<JoinHandle<()>> {
        let accounts = self.accounts.clone()?;
        let passwords = self.passwords.clone();
        let every = self.config.purge_every.max(Duration::from_millis(1));
        Some(tokio::spawn(async move {
            passwords.warm_up().await;
            let mut ticker = tokio::time::interval(every);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = ticker.tick() => match accounts.purge_expired().await {
                        Ok(0) => {}
                        Ok(purged) => tracing::info!(purged, "expired sessions deleted"),
                        Err(error) => tracing::warn!(%error, "could not delete expired sessions"),
                    },
                    _ = stop.changed() => break,
                }
            }
        }))
    }

    fn accounts(&self) -> Result<&Accounts, ApiError> {
        self.accounts.as_ref().ok_or_else(|| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                AuthErrorCode::AccountsUnavailable.as_str(),
                "this server runs without a database, so it has no accounts",
            )
        })
    }

    /// The account behind the request's session cookie. `Ok(None)` when the request has no valid
    /// session (or the server has no accounts); an error when the database fails.
    pub(crate) async fn session_user(&self, headers: &HeaderMap) -> Result<Option<User>, ApiError> {
        let (Some(accounts), Some(token)) = (&self.accounts, cookie::presented(headers)) else {
            return Ok(None);
        };
        accounts
            .session_user(&TokenDigest::of(token))
            .await
            .map_err(unavailable)
    }

    /// The signed-in account, or the error to send: 503 on a server without accounts, 401 when the
    /// request has no valid session.
    pub(crate) async fn require_user(&self, headers: &HeaderMap) -> Result<User, ApiError> {
        self.accounts()?;
        match self.session_user(headers).await? {
            Some(user) => Ok(user),
            None => Err(self.unauthenticated(headers)),
        }
    }

    /// Stores a new session for `user` and returns the `Set-Cookie` value for it. The session the
    /// browser presented, if any, ends here: signing in again replaces it.
    async fn open_session(
        &self,
        accounts: &Accounts,
        user: &User,
        headers: &HeaderMap,
    ) -> Result<HeaderValue, ApiError> {
        let token = SessionToken::generate().map_err(|error| internal(&error))?;
        let lifetime = self.config.session_lifetime;
        accounts
            .start_session(&token.digest(), user.id, lifetime)
            .await
            .map_err(unavailable)?;
        if let Some(old) = cookie::presented(headers)
            && let Err(error) = accounts.end_session(&TokenDigest::of(old)).await
        {
            tracing::warn!(%error, "could not end the session a sign-in replaced");
        }
        cookie::start(&token, lifetime, self.config.cookie_secure)
            .ok_or_else(|| internal(&"the session cookie did not encode"))
    }

    /// The `Set-Cookie` value that clears the cookie.
    fn clearing_cookie(&self) -> Result<HeaderValue, ApiError> {
        cookie::end(self.config.cookie_secure)
            .ok_or_else(|| internal(&"the cleared cookie did not encode"))
    }

    /// The 401 for a request without a valid session; a cookie that was sent is told to go.
    pub(crate) fn unauthenticated(&self, headers: &HeaderMap) -> ApiError {
        let error = failure(
            StatusCode::UNAUTHORIZED,
            AuthErrorCode::Unauthenticated,
            "sign in to do that",
        );
        match (cookie::presented(headers), self.clearing_cookie()) {
            (Some(_), Ok(clear)) => error.with_header(SET_COOKIE, clear),
            _ => error,
        }
    }

    /// Looks `name` up and checks `password` against its hash; an unknown name costs the same
    /// check against a stand-in. `Ok(None)` is a wrong username or a wrong password, and nothing
    /// tells them apart.
    async fn check_login(
        &self,
        accounts: &Accounts,
        name: &Username,
        password: String,
    ) -> Result<Option<User>, ApiError> {
        let stored = accounts.find(&name.key).await.map_err(unavailable)?;
        let hash = stored.as_ref().map(|stored| stored.password_hash.clone());
        let matched = self.passwords.verify(password, hash).await;
        Ok(stored.filter(|_| matched).map(|stored| stored.user))
    }
}

/// `POST /api/auth/register`: makes an account and signs it in.
pub(crate) async fn register(
    State(app): State<AppState>,
    request: Request,
) -> Result<Response, ApiError> {
    let auth = app.auth();
    let accounts = auth.accounts()?;
    if auth.config.registration == Registration::Closed {
        return Err(failure(
            StatusCode::FORBIDDEN,
            AuthErrorCode::RegistrationClosed,
            "this server is not accepting new accounts",
        ));
    }
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let address = client_address(&request, auth.config.trusted_proxies);
    let headers = request.headers().clone();
    let credentials: Credentials = read_json(request).await?;

    let name = Username::parse(&credentials.username).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            AuthErrorCode::InvalidUsername,
            Username::rules(),
        )
    })?;
    check_password(&credentials.password).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            AuthErrorCode::InvalidPassword,
            password_rules(),
        )
    })?;
    let last = auth
        .throttle
        .begin_register(Instant::now(), address)
        .map_err(too_many)?;
    if last {
        tracing::warn!("an address has used up its registrations; more are turned away");
    }

    let hash = auth
        .passwords
        .hash(credentials.password)
        .await
        .map_err(|error| internal(&error))?;
    let user = accounts
        .create_user(&name, &hash)
        .await
        .map_err(|error| match error {
            AccountError::Taken => failure(
                StatusCode::CONFLICT,
                AuthErrorCode::UsernameTaken,
                "that username is taken",
            ),
            error @ AccountError::Unavailable(_) => unavailable(error),
        })?;
    tracing::info!(user = %user.id, "account created");
    let cookie = auth.open_session(accounts, &user, &headers).await?;
    Ok((
        StatusCode::CREATED,
        [(SET_COOKIE, cookie)],
        Json(user_view(&user)),
    )
        .into_response())
}

/// `POST /api/auth/login`: checks the password and signs the account in.
pub(crate) async fn login(
    State(app): State<AppState>,
    request: Request,
) -> Result<Response, ApiError> {
    let auth = app.auth();
    let accounts = auth.accounts()?;
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let address = client_address(&request, auth.config.trusted_proxies);
    let headers = request.headers().clone();
    let credentials: Credentials = read_json(request).await?;

    // A name or password that breaks the rules cannot belong to an account. It gets the answer a
    // wrong password gets, and costs no hashing, so it needs no counting either.
    let Ok(name) = Username::parse(&credentials.username) else {
        return Err(invalid_credentials());
    };
    if check_password(&credentials.password).is_err() {
        return Err(invalid_credentials());
    }

    let attempt = auth
        .throttle
        .begin_login(Instant::now(), address, &name.key)
        .map_err(too_many)?;
    if attempt.exhausted() {
        tracing::warn!("a client has used up its login tries; more are turned away");
    }
    match auth
        .check_login(accounts, &name, credentials.password)
        .await
    {
        Ok(Some(user)) => {
            auth.throttle.succeeded(attempt);
            let cookie = auth.open_session(accounts, &user, &headers).await?;
            tracing::info!(user = %user.id, "signed in");
            Ok((
                StatusCode::OK,
                [(SET_COOKIE, cookie)],
                Json(user_view(&user)),
            )
                .into_response())
        }
        Ok(None) => Err(invalid_credentials()),
        Err(error) => {
            auth.throttle.abandoned(attempt);
            Err(error)
        }
    }
}

/// `POST /api/auth/logout`: ends the session and clears the cookie; fine without a session.
pub(crate) async fn logout(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let auth = app.auth();
    let accounts = auth.accounts()?;
    if let Some(token) = cookie::presented(&headers) {
        accounts
            .end_session(&TokenDigest::of(token))
            .await
            .map_err(unavailable)?;
    }
    let clear = auth.clearing_cookie()?;
    Ok((StatusCode::NO_CONTENT, [(SET_COOKIE, clear)]).into_response())
}

/// `GET /api/auth/me`: the signed-in account; 401 without a valid session (and a stale cookie is
/// cleared on the way out).
pub(crate) async fn me(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserView>, ApiError> {
    let user = app.auth().require_user(&headers).await?;
    Ok(Json(user_view(&user)))
}

fn user_view(user: &User) -> UserView {
    UserView {
        id: user.id.to_string(),
        username: user.username.clone(),
    }
}

fn failure(status: StatusCode, code: AuthErrorCode, message: impl Into<String>) -> ApiError {
    ApiError::new(status, code.as_str(), message)
}

/// One answer for every login that fails on its merits.
fn invalid_credentials() -> ApiError {
    failure(
        StatusCode::UNAUTHORIZED,
        AuthErrorCode::InvalidCredentials,
        "wrong username or password",
    )
}

fn too_many(blocked: Blocked) -> ApiError {
    let seconds = blocked.seconds();
    // Quiet on purpose: a client that keeps knocking would fill the log, and the try that used up
    // the limit was already reported.
    tracing::debug!(
        retry_after = seconds,
        "an account endpoint turned a client away"
    );
    failure(
        StatusCode::TOO_MANY_REQUESTS,
        AuthErrorCode::TooManyAttempts,
        format!("too many attempts; try again in {seconds} seconds"),
    )
    .with_header(RETRY_AFTER, HeaderValue::from(seconds))
}

/// A database failure: the details stay in the log.
#[allow(clippy::needless_pass_by_value)] // an adapter for `map_err`, which hands the error over
fn unavailable(error: AccountError) -> ApiError {
    tracing::error!(%error, "an account request failed");
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        AuthErrorCode::Unavailable,
        "accounts are not available right now; try again later",
    )
}

/// A failure of the server itself: the details stay in the log.
fn internal(reason: &dyn std::fmt::Display) -> ApiError {
    tracing::error!(%reason, "an account request failed inside the server");
    failure(
        StatusCode::INTERNAL_SERVER_ERROR,
        AuthErrorCode::Internal,
        "the request could not be completed",
    )
}

/// The address a request comes from, for counting: the socket's peer, or with
/// `trusted_proxies > 0` the entry `X-Forwarded-For` has that many places from its right end.
pub(crate) fn client_address(request: &Request, trusted_proxies: usize) -> IpAddr {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |ConnectInfo(peer)| {
            peer.ip()
        });
    forwarded_address(request.headers(), peer, trusted_proxies)
}

/// Picks the client out of `X-Forwarded-For`. Every proxy appends the address it saw to the right
/// end, so with `trusted` proxies in front the entry that many places from the right was written
/// by the first of them and cannot be forged by the client; everything to its left can be.
/// Without enough entries, or when the entry is not an address, the peer stands in.
fn forwarded_address(headers: &HeaderMap, peer: IpAddr, trusted: usize) -> IpAddr {
    if trusted == 0 {
        return peer;
    }
    let entries: Vec<&str> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|line| line.to_str().ok())
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .collect();
    let Some(entry) = entries
        .len()
        .checked_sub(trusted)
        .and_then(|at| entries.get(at))
    else {
        return peer;
    };
    entry
        .parse::<IpAddr>()
        .or_else(|_| entry.parse::<SocketAddr>().map(|socket| socket.ip()))
        .unwrap_or(peer)
}

#[cfg(test)]
mod tests {
    //! Goal: the client address is read from `X-Forwarded-For` only as far as the trusted proxies vouch for it, and every malformed or missing header falls back to the socket's peer.
    use super::*;
    use rstest::rstest;

    const PEER: &str = "10.1.1.1";

    fn headers(lines: &[&str]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for line in lines {
            map.append("x-forwarded-for", HeaderValue::from_str(line).unwrap());
        }
        map
    }

    #[rstest]
    #[case::ignored_without_proxies(0, &["203.0.113.9"], PEER)]
    #[case::one_proxy_one_entry(1, &["203.0.113.9"], "203.0.113.9")]
    #[case::forged_prefix_is_ignored(1, &["6.6.6.6, 203.0.113.9"], "203.0.113.9")]
    #[case::forged_prefix_in_another_header(1, &["6.6.6.6", "203.0.113.9"], "203.0.113.9")]
    #[case::two_proxies(2, &["203.0.113.9, 10.2.2.2"], "203.0.113.9")]
    #[case::two_proxies_forged_prefix(2, &["6.6.6.6, 203.0.113.9, 10.2.2.2"], "203.0.113.9")]
    #[case::spaces_and_commas(1, &["  6.6.6.6 ,   203.0.113.9  "], "203.0.113.9")]
    #[case::ipv6(1, &["2001:db8::1"], "2001:db8::1")]
    #[case::with_port(1, &["203.0.113.9:51234"], "203.0.113.9")]
    #[case::ipv6_with_port(1, &["[2001:db8::1]:443"], "2001:db8::1")]
    #[case::no_header(1, &[], PEER)]
    #[case::fewer_entries_than_proxies(2, &["203.0.113.9"], PEER)]
    #[case::not_an_address(1, &["unknown"], PEER)]
    #[case::empty_entry(1, &["203.0.113.9,"], PEER)]
    #[case::garbage_last(1, &["203.0.113.9, nonsense"], PEER)]
    fn the_client_is_the_entry_the_nearest_trusted_proxy_wrote(
        #[case] trusted: usize,
        #[case] lines: &[&str],
        #[case] expected: &str,
    ) {
        let peer: IpAddr = PEER.parse().unwrap();
        let found = forwarded_address(&headers(lines), peer, trusted);
        assert_eq!(found, expected.parse::<IpAddr>().unwrap());
    }

    #[test]
    fn a_header_that_is_not_text_is_skipped() {
        let mut map = HeaderMap::new();
        map.append(
            "x-forwarded-for",
            HeaderValue::from_bytes(b"\xff\xfe").unwrap(),
        );
        map.append("x-forwarded-for", HeaderValue::from_static("203.0.113.9"));
        let peer: IpAddr = PEER.parse().unwrap();
        assert_eq!(
            forwarded_address(&map, peer, 1),
            "203.0.113.9".parse::<IpAddr>().unwrap()
        );
    }
}
