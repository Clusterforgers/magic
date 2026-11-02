//! The account preference endpoints: a signed-in account reads and replaces its pace preferences (the default
//! stops and "End my turn by hand"), which the client applies at each `Welcome`. 503 `cards_unavailable` on a
//! server without a database, then 401 `unauthenticated`.

use axum::Json;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode};
use mtg_db::{PrefsRepo, StoredPrefs};
use mtg_protocol::rest::{DeckErrorCode, MAX_DEFAULT_STOPS, UserPrefs};
use sqlx::types::uuid::Uuid;

use crate::api::{ApiError, read_json_limited};
use crate::card_api::failed;
use crate::server::AppState;

/// Largest body of a preference update: every stop of the list, escaped, with room to spare.
const PREFS_BODY_LIMIT: usize = 8 * 1024;

/// What every preference request has: the signed-in account's row and its id.
struct Session {
    repo: PrefsRepo,
    user: Uuid,
}

/// The 503 of a server without a database, then the 401 of a request without a session.
async fn session(app: &AppState, headers: &HeaderMap) -> Result<Session, ApiError> {
    let cards = app.cards().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            DeckErrorCode::CardsUnavailable.as_str(),
            "this server runs without a database, so it keeps no preferences",
        )
    })?;
    let user = app.auth().require_user(headers).await?;
    Ok(Session {
        repo: cards.database().prefs(),
        user: user.id.uuid(),
    })
}

/// The preferences to store: lasting stops at steps that grant priority, each side and step once, in order.
fn checked(mut prefs: UserPrefs) -> Result<UserPrefs, ApiError> {
    if prefs.default_stops.len() > MAX_DEFAULT_STOPS {
        return Err(ApiError::invalid(format!(
            "default_stops holds at most {MAX_DEFAULT_STOPS} stops"
        )));
    }
    if let Some(stop) = prefs
        .default_stops
        .iter()
        .find(|stop| !stop.keep || !stop.step.grants_priority())
    {
        return Err(ApiError::invalid(format!(
            "a default stop is lasting and at a step with priority, not {stop:?}"
        )));
    }
    prefs.default_stops.sort();
    let before = prefs.default_stops.len();
    prefs
        .default_stops
        .dedup_by_key(|stop| (stop.side, stop.step));
    if prefs.default_stops.len() != before {
        return Err(ApiError::invalid(
            "default_stops names a side and step twice",
        ));
    }
    Ok(prefs)
}

/// `GET /api/me/prefs`: the account's preferences; the defaults (no stops, turns ended by the server) when none were saved.
pub(crate) async fn get(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserPrefs>, ApiError> {
    let s = session(&app, &headers).await?;
    let stored = s.repo.get(s.user).await.map_err(|error| failed(&error))?;
    // A row the server wrote reads back; one that does not (hand-edited) reads as the defaults.
    let default_stops = serde_json::from_value(stored.default_stops).unwrap_or_default();
    Ok(Json(UserPrefs {
        default_stops,
        end_turn_by_hand: stored.end_turn_by_hand,
    }))
}

/// `PUT /api/me/prefs`: replaces the preferences with the body and answers them as stored.
pub(crate) async fn put(
    State(app): State<AppState>,
    request: Request,
) -> Result<Json<UserPrefs>, ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let headers = request.headers().clone();
    let s = session(&app, &headers).await?;
    let body: UserPrefs = read_json_limited(request, PREFS_BODY_LIMIT).await?;
    let prefs = checked(body)?;
    let stored = StoredPrefs {
        default_stops: serde_json::to_value(&prefs.default_stops)
            .map_err(|_| ApiError::unavailable())?,
        end_turn_by_hand: prefs.end_turn_by_hand,
    };
    s.repo
        .put(s.user, &stored)
        .await
        .map_err(|error| failed(&error))?;
    Ok(Json(prefs))
}

#[cfg(test)]
mod tests {
    //! Goal: only lasting stops at steps with priority are stored, each side and step once and in order, and at most the limit.
    use mtg_engine::turn::Step;
    use mtg_protocol::{Stop, StopSide};

    use super::*;

    fn stop(side: StopSide, step: Step, keep: bool) -> Stop {
        Stop { side, step, keep }
    }

    fn prefs(stops: Vec<Stop>) -> UserPrefs {
        UserPrefs {
            default_stops: stops,
            end_turn_by_hand: false,
        }
    }

    #[test]
    fn good_stops_are_stored_in_order() {
        let got = checked(prefs(vec![
            stop(StopSide::Opponent, Step::Upkeep, true),
            stop(StopSide::Own, Step::Upkeep, true),
        ]))
        .unwrap();
        assert_eq!(got.default_stops[0].side, StopSide::Own);
    }

    #[test]
    fn one_turn_stops_and_priorityless_steps_are_refused() {
        assert!(checked(prefs(vec![stop(StopSide::Own, Step::Upkeep, false)])).is_err());
        assert!(checked(prefs(vec![stop(StopSide::Own, Step::Untap, true)])).is_err());
    }

    #[test]
    fn a_place_twice_and_too_many_are_refused() {
        let one = stop(StopSide::Own, Step::Upkeep, true);
        assert!(checked(prefs(vec![one, one])).is_err());
        assert!(checked(prefs(vec![one; MAX_DEFAULT_STOPS + 1])).is_err());
    }
}
