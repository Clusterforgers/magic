//! `GET /api/me/games`: the games the signed-in user has a seat in, running, in a lobby, finished
//! or failed to resume, newest first. A bound seat is rejoined with the session alone
//! (`GET /ws/{game}`). `DELETE /api/me/games/{id}` takes a failed game off the list.

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use mtg_core::ids::PlayerId;
use mtg_protocol::rest::{AuthErrorCode, MyGame, MyGameSeat, MyGameStatus, MyGames};

use crate::api::ApiError;
use crate::game_id::GameId;
use crate::records::{RecordStatus, UserGame};
use crate::registry::Cause;
use crate::server::AppState;

/// The handler.
pub(crate) async fn list(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MyGames>, ApiError> {
    let user = app.auth().require_user(&headers).await?;
    let games = app.registry().games_of(user.id).await.map_err(|error| {
        tracing::error!(%error, "could not list a user's games");
        unavailable("your games are not available right now; try again later")
    })?;
    Ok(Json(MyGames {
        games: games.into_iter().filter_map(view).collect(),
    }))
}

/// `DELETE /api/me/games/{id}`: takes a game that failed to resume off the signed-in user's list;
/// 204. The other players keep it. A game that is not in the list, did not fail or was removed
/// already is 404, so nothing tells a stranger which games exist.
pub(crate) async fn remove(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let user = app.auth().require_user(&headers).await?;
    let game: GameId = path
        .ok()
        .and_then(|Path(id)| id.parse().ok())
        .ok_or_else(ApiError::not_found)?;
    let removed = app
        .registry()
        .remove_failed(user.id, game)
        .await
        .map_err(|error| {
            tracing::error!(%error, "could not remove a failed game from a user's list");
            unavailable("the game could not be removed right now; try again later")
        })?;
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found())
    }
}

fn unavailable(message: &str) -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        AuthErrorCode::Unavailable.as_str(),
        message,
    )
}

/// A listed game as the wire shows it; `None` for a status that is never listed.
fn view(game: UserGame) -> Option<MyGame> {
    let (status, reason) = match game.status {
        RecordStatus::Lobby => (MyGameStatus::Lobby, None),
        RecordStatus::Running => (MyGameStatus::Running, None),
        RecordStatus::Finished => (MyGameStatus::Finished, None),
        // A game marked failed before the reason was kept still says something plain.
        RecordStatus::Failed => (
            MyGameStatus::Failed,
            Some(game.reason.unwrap_or_else(|| Cause::Unknown.reason())),
        ),
        RecordStatus::Expired => return None,
    };
    let name = game
        .seats
        .iter()
        .find(|seat| seat.seat == game.seat)
        .map_or_else(
            || format!("Seat {}", game.seat + 1),
            |seat| seat.name.clone(),
        );
    let others = game
        .seats
        .into_iter()
        .filter(|seat| seat.seat != game.seat)
        .map(|seat| MyGameSeat {
            seat: PlayerId::from_index(seat.seat),
            name: seat.name,
            user: seat.user,
        })
        .collect();
    Some(MyGame {
        game: game.id.to_string(),
        seat: PlayerId::from_index(game.seat),
        name,
        status,
        created_at: game.created_at,
        others,
        result: game.result,
        reason,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a stored game becomes the wire shape with the user's own seat split from the others in seat order, a result only where there is one, a reason only on a failed game (a plain one when none was kept), and games that were dropped are never shown.
    use mtg_engine::state::GameResult;
    use rstest::rstest;
    use uuid::Uuid;

    use super::*;
    use crate::game_id::GameId;
    use crate::records::UserGameSeat;

    const STORED: &str =
        "The deck of Bo has changed since this game was made, so it cannot continue.";

    fn seat(seat: usize, name: &str, user: Option<&str>) -> UserGameSeat {
        UserGameSeat {
            seat,
            name: name.into(),
            user: user.map(str::to_owned),
        }
    }

    fn game(status: RecordStatus, mine: usize, seats: Vec<UserGameSeat>) -> UserGame {
        UserGame {
            id: GameId(Uuid::from_u128(7)),
            seat: mine,
            status,
            created_at: "2026-10-18T12:00:00Z".into(),
            result: matches!(status, RecordStatus::Finished)
                .then_some(GameResult::Winner(PlayerId::from_index(1))),
            reason: matches!(status, RecordStatus::Failed).then(|| STORED.to_owned()),
            seats,
        }
    }

    #[rstest]
    #[case(RecordStatus::Lobby, Some(MyGameStatus::Lobby))]
    #[case(RecordStatus::Running, Some(MyGameStatus::Running))]
    #[case(RecordStatus::Finished, Some(MyGameStatus::Finished))]
    #[case(RecordStatus::Expired, None)]
    #[case(RecordStatus::Failed, Some(MyGameStatus::Failed))]
    fn only_games_that_were_not_dropped_are_shown(
        #[case] status: RecordStatus,
        #[case] expected: Option<MyGameStatus>,
    ) {
        let listed = view(game(
            status,
            0,
            vec![seat(0, "Ann", Some("ann")), seat(1, "Bo", None)],
        ));
        assert_eq!(listed.map(|g| g.status), expected);
    }

    #[rstest]
    #[case::first(0, "Ann", &[(1, "Bo", None), (2, "Cy", Some("cyrus"))])]
    #[case::middle(1, "Bo", &[(0, "Ann", Some("ann")), (2, "Cy", Some("cyrus"))])]
    #[case::last(2, "Cy", &[(0, "Ann", Some("ann")), (1, "Bo", None)])]
    fn the_users_own_seat_is_split_from_the_others_in_seat_order(
        #[case] mine: usize,
        #[case] name: &str,
        #[case] others: &[(usize, &str, Option<&str>)],
    ) {
        let seats = vec![
            seat(0, "Ann", Some("ann")),
            seat(1, "Bo", None),
            seat(2, "Cy", Some("cyrus")),
        ];
        let listed = view(game(RecordStatus::Running, mine, seats)).unwrap();
        assert_eq!(listed.seat, PlayerId::from_index(mine));
        assert_eq!(listed.name, name);
        let found: Vec<_> = listed
            .others
            .iter()
            .map(|other| {
                (
                    other.seat.index(),
                    other.name.as_str(),
                    other.user.as_deref(),
                )
            })
            .collect();
        assert_eq!(found, others);
        assert_eq!(listed.game, "00000000-0000-0000-0000-000000000007");
        assert_eq!(listed.created_at, "2026-10-18T12:00:00Z");
        assert_eq!(listed.result, None);
    }

    #[test]
    fn a_finished_game_carries_its_result_and_no_other_does() {
        let seats = || vec![seat(0, "Ann", None), seat(1, "Bo", None)];
        let finished = view(game(RecordStatus::Finished, 0, seats())).unwrap();
        assert_eq!(
            finished.result,
            Some(GameResult::Winner(PlayerId::from_index(1)))
        );
        for status in [RecordStatus::Lobby, RecordStatus::Running] {
            assert_eq!(view(game(status, 0, seats())).unwrap().result, None);
        }
    }

    #[test]
    fn a_failed_game_carries_its_stored_reason_and_no_other_does() {
        let seats = || vec![seat(0, "Ann", None), seat(1, "Bo", None)];
        let failed = view(game(RecordStatus::Failed, 0, seats())).unwrap();
        assert_eq!(failed.reason.as_deref(), Some(STORED));
        assert_eq!(failed.result, None);
        for status in [
            RecordStatus::Lobby,
            RecordStatus::Running,
            RecordStatus::Finished,
        ] {
            assert_eq!(view(game(status, 0, seats())).unwrap().reason, None);
        }
    }

    #[test]
    fn a_game_failed_before_reasons_were_kept_says_so_plainly() {
        let mut old = game(RecordStatus::Failed, 0, vec![seat(0, "Ann", None)]);
        old.reason = None;
        let reason = view(old).unwrap().reason.unwrap();
        assert_eq!(
            reason,
            "This game could not be restored, so it cannot continue."
        );
    }

    #[test]
    fn a_damaged_record_without_the_users_own_seat_still_lists_a_name() {
        let listed = view(game(RecordStatus::Running, 3, vec![seat(0, "Ann", None)])).unwrap();
        assert_eq!(listed.name, "Seat 4");
        assert_eq!(listed.others.len(), 1);
    }
}
