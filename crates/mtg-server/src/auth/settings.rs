//! Account settings: `POST /api/auth/password` and `DELETE /api/auth/account`.
//!
//! Both act on the signed-in account and are confirmed with its current password, which is
//! checked and counted exactly as a login is (same counters, same Argon2 cost, same 429), so a
//! stolen session cannot be used to guess the password any faster than the sign-in form could.

use std::net::IpAddr;
use std::time::Instant;

use axum::extract::{Request, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use mtg_protocol::rest::{AccountDeletion, AuthErrorCode, PasswordChange};

use super::admin::admin_cannot_be_deleted;
use super::{Auth, client_address, failure, internal, too_many, unavailable};
use crate::accounts::{Accounts, Outcome, User};
use crate::api::{ApiError, read_credentials};
use crate::cookie;
use crate::credentials::{check_password, password_rules};
use crate::secret::TokenDigest;
use crate::server::AppState;

/// Checks that `password` is the one of `user`. A password that breaks the rules cannot be it and
/// is refused unhashed and uncounted, like a login. Otherwise the try is counted until the answer
/// is known: right, it is handed back; wrong, it stays counted.
async fn confirm(
    auth: &Auth,
    accounts: &Accounts,
    user: &User,
    headers: &HeaderMap,
    address: IpAddr,
    password: String,
) -> Result<(), ApiError> {
    if check_password(&password).is_err() {
        return Err(wrong_password());
    }
    let attempt = auth
        .throttle
        .begin_login(Instant::now(), address, &user.username.to_ascii_lowercase())
        .map_err(too_many)?;
    if attempt.exhausted() {
        tracing::warn!("a client has used up its password tries; more are turned away");
    }
    let hash = match accounts.password_hash(user.id).await {
        Ok(Some(hash)) => hash,
        // The account went between the session check and now.
        Ok(None) => {
            auth.throttle.abandoned(attempt);
            return Err(auth.unauthenticated(headers));
        }
        Err(error) => {
            auth.throttle.abandoned(attempt);
            return Err(unavailable(error));
        }
    };
    if auth.passwords.verify(password, Some(hash)).await {
        auth.throttle.succeeded(attempt);
        Ok(())
    } else {
        Err(wrong_password())
    }
}

fn wrong_password() -> ApiError {
    // The code and status of a wrong sign-in; the message says it was the password.
    failure(
        StatusCode::UNAUTHORIZED,
        AuthErrorCode::InvalidCredentials,
        "wrong password",
    )
}

/// `POST /api/auth/password`: replaces the password of the signed-in account. The sessions of the
/// account other than this one end, so a password changed because it leaked also locks out
/// whoever holds a stolen cookie.
pub(crate) async fn change_password(
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
    let user = auth.require_user(&headers).await?;
    let body: PasswordChange = read_credentials(request).await?;

    check_password(&body.new_password).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            AuthErrorCode::InvalidPassword,
            password_rules(),
        )
    })?;
    confirm(
        auth,
        accounts,
        &user,
        &headers,
        address,
        body.current_password,
    )
    .await?;

    let hash = auth
        .passwords
        .hash(body.new_password)
        .await
        .map_err(|error| internal(&error))?;
    let current = cookie::presented(&headers)
        .map(TokenDigest::of)
        .ok_or_else(|| auth.unauthenticated(&headers))?;
    if !accounts
        .change_password(user.id, &hash, &current)
        .await
        .map_err(unavailable)?
    {
        return Err(auth.unauthenticated(&headers));
    }
    tracing::info!(user = %user.id, "password changed; the other sessions ended");
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// `DELETE /api/auth/account`: deletes the signed-in account. Its decks, sessions and seat
/// bindings go in one transaction; then it leaves the live games ([`Registry::forget_user`]) and
/// the cookie is cleared. The admin's account is refused with 409
/// `admin_account`, since nothing makes another one.
///
/// [`Registry::forget_user`]: crate::registry::Registry::forget_user
pub(crate) async fn delete_account(
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
    let user = auth.require_user(&headers).await?;
    // Decided before the body is read: nothing makes another admin, so this one stays.
    if user.is_admin {
        return Err(admin_cannot_be_deleted());
    }
    let body: AccountDeletion = read_credentials(request).await?;

    confirm(auth, accounts, &user, &headers, address, body.password).await?;
    match accounts.delete_user(user.id).await.map_err(unavailable)? {
        Outcome::Done => {}
        Outcome::Gone => return Err(auth.unauthenticated(&headers)),
        Outcome::Admin => return Err(admin_cannot_be_deleted()),
    }
    tracing::info!(user = %user.id, "account deleted");
    app.registry().forget_user(user.id).await;
    let clear = auth.clearing_cookie()?;
    Ok((StatusCode::NO_CONTENT, [(SET_COOKIE, clear)]).into_response())
}
