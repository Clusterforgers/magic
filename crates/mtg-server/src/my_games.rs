//! `GET /api/me/games`: the games the signed-in user has a seat in, running, in a lobby or
//! finished, newest first. A bound seat is rejoined with the session alone (`GET /ws/{game}`).

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use mtg_core::ids::PlayerId;
use mtg_protocol::rest::{AuthErrorCode, MyGame, MyGameSeat, MyGameStatus, MyGames};

use crate::api::ApiError;
use crate::records::{RecordStatus, UserGame};
use crate::server::AppState;

/// The handler.
pub(crate) async fn list(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MyGames>, ApiError> {
    let user = app.auth().require_user(&headers).await?;
    let games = app.registry().games_of(user.id).await.map_err(|error| {
        tracing::error!(%error, "could not list a user's games");
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            AuthErrorCode::Unavailable.as_str(),
            "your games are not available right now; try again later",
        )
    })?;
    Ok(Json(MyGames {
        games: games.into_iter().filter_map(view).collect(),
    }))
}

/// A listed game as the wire shows it; `None` for a status that is never listed.
fn view(game: UserGame) -> Option<MyGame> {
    let status = match game.status {
        RecordStatus::Lobby => MyGameStatus::Lobby,
        RecordStatus::Running => MyGameStatus::Running,
        RecordStatus::Finished => MyGameStatus::Finished,
        RecordStatus::Expired | RecordStatus::Failed => return None,
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
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a stored game becomes the wire shape with the user's own seat split from the others in seat order, a result only where there is one, and games that are dropped or damaged are never shown.
    use mtg_engine::state::GameResult;
    use rstest::rstest;
    use uuid::Uuid;

    use super::*;
    use crate::game_id::GameId;
    use crate::records::UserGameSeat;

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
            seats,
        }
    }

    #[rstest]
    #[case(RecordStatus::Lobby, Some(MyGameStatus::Lobby))]
    #[case(RecordStatus::Running, Some(MyGameStatus::Running))]
    #[case(RecordStatus::Finished, Some(MyGameStatus::Finished))]
    #[case(RecordStatus::Expired, None)]
    #[case(RecordStatus::Failed, None)]
    fn only_games_that_can_be_played_or_were_finished_are_shown(
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
    fn a_damaged_record_without_the_users_own_seat_still_lists_a_name() {
        let listed = view(game(RecordStatus::Running, 3, vec![seat(0, "Ann", None)])).unwrap();
        assert_eq!(listed.name, "Seat 4");
        assert_eq!(listed.others.len(), 1);
    }
}
