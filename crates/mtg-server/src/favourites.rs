//! The favourite-precon endpoints: a signed-in account stars a precon, takes the star back and lists its
//! stars. A favourite is a pointer to the precon (`precon:<code>` as a deck), never a copy, so it follows
//! the precon's cards and check. Every route needs an account: 503 `cards_unavailable` on a server without
//! a database, then 401 `unauthenticated`. Stars and unstars count per account (`AttemptLimits::favourite_writes_per_user`).

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use mtg_db::{FavouritePreconRepo, is_code};
use mtg_protocol::rest::{DeckErrorCode, FavouritePrecons};

use crate::api::ApiError;
use crate::card_api::failed;
use crate::cards::CardSource;
use crate::server::AppState;
use crate::user_id::UserId;

/// What every favourite request has: the card source, the signed-in account's stars and its id.
struct Session<'a> {
    cards: &'a CardSource,
    repo: FavouritePreconRepo,
    user: UserId,
}

/// The 503 of a server without a database, then the 401 of a request without a session.
async fn session<'a>(app: &'a AppState, headers: &HeaderMap) -> Result<Session<'a>, ApiError> {
    let cards = app.cards().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            DeckErrorCode::CardsUnavailable.as_str(),
            "this server runs without a database, so it has no precons to star",
        )
    })?;
    let user = app.auth().require_user(headers).await?;
    Ok(Session {
        cards,
        repo: cards.database().favourite_precons(),
        user: user.id,
    })
}

/// The code in the path, in lower case; one that has no code's shape and one nobody has look alike.
fn code_of(path: Result<Path<String>, PathRejection>) -> Result<String, ApiError> {
    let Path(code) = path.map_err(|_| ApiError::not_found())?;
    let code = code.to_ascii_lowercase();
    if is_code(&code) {
        Ok(code)
    } else {
        Err(ApiError::not_found())
    }
}

/// `GET /api/favourites/precons`: the account's stars that name a precon the server has, the newest first.
pub(crate) async fn list(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<FavouritePrecons>, ApiError> {
    let s = session(&app, &headers).await?;
    let stars = s
        .repo
        .list(s.user.uuid())
        .await
        .map_err(|error| failed(&error))?;
    let index = s.cards.precon_index().await?;
    let codes = stars
        .into_iter()
        .filter(|code| index.find(code).is_some())
        .collect();
    Ok(Json(FavouritePrecons { codes }))
}

/// `PUT /api/favourites/precons/{code}`: stars the precon; 204, also when it was starred. A code nobody has
/// is 404 `not_found`.
pub(crate) async fn put(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let s = session(&app, &headers).await?;
    app.auth().admit_favourite_write(s.user)?;
    let code = code_of(path)?;
    if s.cards.precon_index().await?.find(&code).is_none() {
        return Err(ApiError::not_found());
    }
    s.repo
        .add(s.user.uuid(), &code)
        .await
        .map_err(|error| failed(&error))?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/favourites/precons/{code}`: takes the star back; 204, also when there was none, so a star of
/// a precon that is gone can be cleared.
pub(crate) async fn remove(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let s = session(&app, &headers).await?;
    app.auth().admit_favourite_write(s.user)?;
    let code = code_of(path)?;
    s.repo
        .remove(s.user.uuid(), &code)
        .await
        .map_err(|error| failed(&error))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    //! Goal: a path that is no precon code is a plain 404, and a code in capitals is the code in lower case.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("")]
    #[case("Not A Code")]
    #[case("precon:c18")]
    #[case("-lead")]
    fn paths_that_are_no_codes_are_404(#[case] code: &str) {
        let error = code_of(Ok(Path(code.to_owned()))).unwrap_err();
        assert_eq!(error.status(), StatusCode::NOT_FOUND);
        assert_eq!(error.code(), "not_found");
    }

    #[test]
    fn a_code_in_capitals_is_the_code_in_lower_case() {
        assert_eq!(
            code_of(Ok(Path("C18-Adaptive-Enchantment".to_owned()))).unwrap(),
            "c18-adaptive-enchantment"
        );
    }
}
