//! The admin routes: `POST /api/admin/users` makes an account and `GET /api/admin/users` lists them,
//! `POST /api/admin/users/{id}/password` sets an account's password and `DELETE /api/admin/users/{id}`
//! removes an account, and `GET /api/auth/options` tells a visitor whether registering is open.
//!
//! The admin is the one account `MTG_ADMIN_USERNAME` names ([`Auth::sync_admin`] marks it at start).
//! Every admin request reads the flag from the database for the session's account, so a demotion
//! takes effect on the next request and neither the cookie nor the client says who is the admin.
//! The checks run in a fixed order, all before the body is read: 503 `accounts_unavailable`, 401
//! `unauthenticated`, 403 `forbidden`; the routes that name an account then answer 404
//! `unknown_account` and 409 `admin_account`.

use std::time::Instant;

use axum::Json;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use mtg_protocol::rest::{
    AccountList, AccountOptions, AccountRow, AuthErrorCode, CreatedAccount, Credentials,
    PasswordReset,
};

use super::{Auth, failure, internal, too_many, unavailable};
use crate::accounts::{AccountError, Accounts, Outcome, User};
use crate::api::{ApiError, read_credentials};
use crate::config::Registration;
use crate::credentials::{Username, check_password, password_rules};
use crate::server::AppState;
use crate::user_id::UserId;

impl Auth {
    /// Marks the account `MTG_ADMIN_USERNAME` names as the admin and clears the flag of every other
    /// account, in one statement. An unset name demotes everyone; a name that belongs to no account
    /// yet changes nothing else (the owner registers first, then names the account). A server
    /// without a database has no accounts and does nothing.
    pub(crate) async fn sync_admin(&self) -> Result<(), AccountError> {
        let Some(accounts) = &self.accounts else {
            return Ok(());
        };
        let admins = accounts.set_admin(self.config.admin.as_deref()).await?;
        match (self.config.admin.is_some(), admins) {
            (false, _) => {
                tracing::info!("no admin account is configured; nobody can make accounts");
            }
            (true, 0) => tracing::warn!(
                "MTG_ADMIN_USERNAME names no account yet; nobody can make accounts until it exists"
            ),
            (true, _) => tracing::info!("the admin account is marked"),
        }
        Ok(())
    }

    /// The signed-in admin, or the error to send: 503 on a server without accounts, 401 without a
    /// valid session, 403 `forbidden` for an account that is not the admin.
    async fn require_admin(&self, headers: &HeaderMap) -> Result<User, ApiError> {
        let user = self.require_user(headers).await?;
        if user.is_admin {
            Ok(user)
        } else {
            Err(failure(
                StatusCode::FORBIDDEN,
                AuthErrorCode::Forbidden,
                "only the admin can do that",
            ))
        }
    }
}

/// `POST /api/admin/users`: makes an account for someone else. It starts no session, neither for
/// the admin's browser nor for the new account, and works whether registration is open or closed.
/// The admin pays for the hashing out of a budget of its own, not out of the address's
/// registrations.
pub(crate) async fn create_account(
    State(app): State<AppState>,
    request: Request,
) -> Result<Response, ApiError> {
    let auth = app.auth();
    let accounts = auth.accounts()?;
    let admin = auth.require_admin(request.headers()).await?;
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let credentials: Credentials = read_credentials(request).await?;

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
    auth.throttle
        .begin_admin_create(Instant::now(), admin.id)
        .map_err(too_many)?;

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
    tracing::info!(admin = %admin.id, user = %user.id, "account created by the admin");
    Ok((
        StatusCode::CREATED,
        Json(CreatedAccount {
            id: user.id.to_string(),
            username: user.username,
        }),
    )
        .into_response())
}

/// `GET /api/admin/users`: every account, oldest first, with nothing that opens it.
pub(crate) async fn list_accounts(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AccountList>, ApiError> {
    let auth = app.auth();
    let accounts = auth.accounts()?;
    auth.require_admin(&headers).await?;
    let users = accounts
        .list_users()
        .await
        .map_err(unavailable)?
        .into_iter()
        .map(|listed| AccountRow {
            id: listed.id.to_string(),
            username: listed.username,
            created_at: listed.created_at,
            is_admin: listed.is_admin,
        })
        .collect();
    Ok(Json(AccountList { users }))
}

/// `GET /api/auth/options`: whether registering is open, for a sign-up page that would rather say
/// so than fail after the form is filled in. Open to anyone, and 503 on a server without accounts.
pub(crate) async fn options(State(app): State<AppState>) -> Result<Json<AccountOptions>, ApiError> {
    let auth = app.auth();
    auth.accounts()?;
    Ok(Json(AccountOptions {
        registration_open: auth.config.registration == Registration::Open,
    }))
}

/// The account an admin route names, or the 404 to send: a text that is not an id names none.
async fn named_account(accounts: &Accounts, id: &str) -> Result<User, ApiError> {
    let id: UserId = id.parse().map_err(|_| unknown_account())?;
    accounts
        .find_by_id(id)
        .await
        .map_err(unavailable)?
        .ok_or_else(unknown_account)
}

fn unknown_account() -> ApiError {
    failure(
        StatusCode::NOT_FOUND,
        AuthErrorCode::UnknownAccount,
        "there is no such account",
    )
}

/// The 409 for the admin's account, which neither the admin routes nor Account settings delete.
pub(super) fn admin_cannot_be_deleted() -> ApiError {
    failure(
        StatusCode::CONFLICT,
        AuthErrorCode::AdminAccount,
        "this account makes accounts for your friends, so it cannot be deleted",
    )
}

fn admin_password_is_in_settings() -> ApiError {
    failure(
        StatusCode::CONFLICT,
        AuthErrorCode::AdminAccount,
        "this is the admin account; change its password under Account settings",
    )
}

/// `POST /api/admin/users/{id}/password`: sets the password of an account that forgot it. Every
/// session of the account ends, as in a password change. The admin's own password changes only in
/// Account settings. The hashing comes out of the admin's budget, like `create_account`.
pub(crate) async fn reset_password(
    State(app): State<AppState>,
    Path(id): Path<String>,
    request: Request,
) -> Result<StatusCode, ApiError> {
    let auth = app.auth();
    let accounts = auth.accounts()?;
    let admin = auth.require_admin(request.headers()).await?;
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let target = named_account(accounts, &id).await?;
    if target.is_admin {
        return Err(admin_password_is_in_settings());
    }
    let body: PasswordReset = read_credentials(request).await?;
    check_password(&body.password).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            AuthErrorCode::InvalidPassword,
            password_rules(),
        )
    })?;
    auth.throttle
        .begin_admin_create(Instant::now(), admin.id)
        .map_err(too_many)?;

    let hash = auth
        .passwords
        .hash(body.password)
        .await
        .map_err(|error| internal(&error))?;
    match accounts
        .reset_password(target.id, &hash)
        .await
        .map_err(unavailable)?
    {
        Outcome::Done => {}
        Outcome::Gone => return Err(unknown_account()),
        Outcome::Admin => return Err(admin_password_is_in_settings()),
    }
    tracing::info!(admin = %admin.id, user = %target.id, "password set by the admin; the sessions ended");
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/admin/users/{id}`: removes an account that is not the admin's, in the transaction
/// of [`Accounts::delete_user`], then takes it out of the live games as `DELETE /api/auth/account`
/// does. It counts against the admin's budget.
pub(crate) async fn remove_account(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let auth = app.auth();
    let accounts = auth.accounts()?;
    let admin = auth.require_admin(&headers).await?;
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let target = named_account(accounts, &id).await?;
    if target.is_admin {
        return Err(admin_cannot_be_deleted());
    }
    auth.throttle
        .begin_admin_create(Instant::now(), admin.id)
        .map_err(too_many)?;
    match accounts.delete_user(target.id).await.map_err(unavailable)? {
        Outcome::Done => {}
        Outcome::Gone => return Err(unknown_account()),
        Outcome::Admin => return Err(admin_cannot_be_deleted()),
    }
    tracing::info!(admin = %admin.id, user = %target.id, "account removed by the admin");
    app.registry().forget_user(target.id).await;
    Ok(StatusCode::NO_CONTENT)
}
