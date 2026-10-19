//! Goal: an open seat is claimed by the first visitor who asks and never by two, an account holds one seat per game and is handed its own again, the entrance decides which games a visitor can reach, a claim names the seat at the table and a release empties it again (token, connection, Ready and account), only the holder and the host may release, and once the game starts claims, releases and the visibility are refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::HashSet;

use common::{Client, seat, silent};
use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, LobbySeat, ServerMessage};
use mtg_server::registry::{
    ClaimError, Created, Entrance, GameOptions, HOST, SeatClaim, SeatRequest, Seated, TableError,
};
use mtg_server::{GameHandle, GameId, Registry, SeatState, ServerConfig, UserId, Visibility, demo};
use rstest::rstest;

fn requests(seats: usize) -> Vec<SeatRequest> {
    (0..seats)
        .map(|n| SeatRequest {
            name: format!("Player {n}"),
            deck: &demo::all()[n % demo::all().len()],
        })
        .collect()
}

fn registry() -> Registry {
    Registry::new(&ServerConfig::default())
}

/// A lobby of `seats` seats of which only the host's is held.
async fn lobby(registry: &Registry, seats: usize, visibility: Visibility) -> Created {
    let options = GameOptions {
        visibility,
        held: Some(1),
    };
    registry
        .create_with(&requests(seats), options, None)
        .await
        .unwrap()
}

fn guest(name: &str) -> SeatClaim {
    SeatClaim {
        name: name.to_owned(),
        user: None,
    }
}

fn account(name: &str, user: UserId) -> SeatClaim {
    SeatClaim {
        name: name.to_owned(),
        user: Some(user),
    }
}

async fn join(registry: &Registry, game: GameId, claim: SeatClaim) -> Result<Seated, ClaimError> {
    registry.claim(game, Entrance::Code, claim).await
}

/// The state of every seat of `game`, in seat order.
async fn states(registry: &Registry, game: GameId) -> Vec<SeatState> {
    let public = registry.describe(game).await.unwrap();
    public.seats.iter().map(|seat| seat.state).collect()
}

/// The names at every seat of `game`, in seat order.
async fn names(registry: &Registry, game: GameId) -> Vec<String> {
    let public = registry.describe(game).await.unwrap();
    public.seats.into_iter().map(|seat| seat.name).collect()
}

/// The roster a lobby sends: each seat's name, whether a client is connected and whether it is ready.
fn roster(rows: &[(&str, bool, bool)]) -> ServerMessage {
    let seats = rows
        .iter()
        .enumerate()
        .map(|(index, (name, connected, ready))| LobbySeat {
            seat: seat(index),
            name: (*name).to_owned(),
            connected: *connected,
            ready: *ready,
        })
        .collect();
    let needed = rows.iter().filter(|(_, _, ready)| !ready).count();
    ServerMessage::Lobby {
        seats,
        needed: u32::try_from(needed).unwrap(),
    }
}

/// Connects the host and throws away what the join sent.
async fn host_of(handle: &GameHandle) -> Client {
    let mut host = Client::join(handle, 0).await;
    host.next().await;
    host.next().await;
    host.drain();
    host
}

/// Waits until the actor has dealt with everything sent to it so far.
async fn settle(handle: &GameHandle) {
    handle.status().await.unwrap();
}

#[tokio::test]
async fn claims_take_the_open_seats_in_order_and_a_token_opens_only_its_own() {
    let registry = registry();
    let created = lobby(&registry, 3, Visibility::Public).await;
    let game = created.id;
    assert_eq!(
        states(&registry, game).await,
        [SeatState::Issued, SeatState::Open, SeatState::Open]
    );

    let first = join(&registry, game, guest("Bo")).await.unwrap();
    let second = join(&registry, game, guest("Cy")).await.unwrap();
    assert_eq!((first.seat, second.seat), (seat(1), seat(2)));
    assert_eq!((first.game, second.game), (game, game));
    let (first_token, second_token) = (first.token.unwrap(), second.token.unwrap());
    assert_ne!(first_token.expose(), second_token.expose());

    assert_eq!(
        registry
            .authenticate(game, first_token.expose())
            .unwrap()
            .seat,
        seat(1)
    );
    assert_eq!(
        registry
            .authenticate(game, second_token.expose())
            .unwrap()
            .seat,
        seat(2)
    );
    assert_eq!(
        states(&registry, game).await,
        [SeatState::Issued, SeatState::Claimed, SeatState::Claimed]
    );
    assert_eq!(names(&registry, game).await, ["Player 0", "Bo", "Cy"]);

    // The creator's own token still opens seat 0, and the lobby is full now.
    let (_, host_token) = &created.seats[0];
    assert_eq!(
        registry
            .authenticate(game, host_token.expose())
            .unwrap()
            .seat,
        seat(0)
    );
    assert_eq!(
        join(&registry, game, guest("Di")).await.unwrap_err(),
        ClaimError::Full
    );
}

#[tokio::test]
async fn a_claim_and_a_pre_issued_token_are_told_apart_in_the_model() {
    let registry = registry();
    let options = GameOptions {
        visibility: Visibility::Public,
        held: Some(2),
    };
    let created = registry
        .create_with(&requests(3), options, None)
        .await
        .unwrap();
    // Seat 1's token was made with the game; seat 2 is claimable.
    assert_eq!(created.seats.len(), 2);
    assert_eq!(
        states(&registry, created.id).await,
        [SeatState::Issued, SeatState::Issued, SeatState::Open]
    );
    join(&registry, created.id, guest("Cy")).await.unwrap();
    assert_eq!(
        states(&registry, created.id).await,
        [SeatState::Issued, SeatState::Issued, SeatState::Claimed]
    );
}

#[rstest]
#[case::plain("Bo", Ok("Bo"))]
#[case::trimmed("  Bo\t", Ok("Bo"))]
#[case::spaces_inside("Bo Peep", Ok("Bo Peep"))]
#[case::unicode("Zoë 日本", Ok("Zoë 日本"))]
#[case::longest("x".repeat(32).leak(), Ok("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"))]
#[case::too_long("x".repeat(33).leak(), Err(ClaimError::BadName))]
#[case::empty("", Err(ClaimError::BadName))]
#[case::blank("  \t ", Err(ClaimError::BadName))]
#[tokio::test]
async fn a_guest_gives_a_name_and_nothing_else(
    #[case] name: &str,
    #[case] expected: Result<&str, ClaimError>,
) {
    let registry = registry();
    let created = lobby(&registry, 2, Visibility::Public).await;
    let claimed = join(&registry, created.id, guest(name)).await;
    match expected {
        Ok(shown) => {
            assert_eq!(claimed.unwrap().seat, seat(1));
            assert_eq!(names(&registry, created.id).await[1], shown);
        }
        Err(error) => {
            assert_eq!(claimed.unwrap_err(), error);
            // A refused name leaves the seat open.
            assert_eq!(states(&registry, created.id).await[1], SeatState::Open);
        }
    }
}

#[tokio::test]
async fn guests_have_no_identity_so_two_may_share_a_name_and_one_may_take_several_seats() {
    let registry = registry();
    let created = lobby(&registry, 4, Visibility::Public).await;
    let seats: Vec<Seated> = vec![
        join(&registry, created.id, guest("Bo")).await.unwrap(),
        join(&registry, created.id, guest("Bo")).await.unwrap(),
        join(&registry, created.id, guest("Bo")).await.unwrap(),
    ];
    let numbers: Vec<_> = seats.iter().map(|s| s.seat.index()).collect();
    assert_eq!(numbers, [1, 2, 3]);
    assert!(seats.iter().all(|s| s.token.is_some()));
    assert_eq!(
        names(&registry, created.id).await,
        ["Player 0", "Bo", "Bo", "Bo"]
    );
}

#[tokio::test]
async fn an_account_holds_one_seat_per_game_and_is_handed_its_own_again() {
    let registry = registry();
    let (host, ann, bob) = (UserId::random(), UserId::random(), UserId::random());
    let options = GameOptions {
        visibility: Visibility::Public,
        held: Some(1),
    };
    // A game without a database has no accounts to bind at creation, so the host is a guest here.
    let created = registry
        .create_with(&requests(4), options, Some(host))
        .await
        .unwrap();
    let game = created.id;

    let first = join(&registry, game, account("Ann", ann)).await.unwrap();
    assert_eq!(first.seat, seat(1));
    assert!(first.token.is_some());
    // Asking again returns the same seat and no second token, however often and whatever name.
    for name in ["Ann", "Anna"] {
        let again = join(&registry, game, account(name, ann)).await;
        let again = again.unwrap_or_else(|error| panic!("{name:?}: {error}"));
        assert_eq!((again.game, again.seat, again.token), (game, seat(1), None));
    }
    // A request that is not valid is refused whoever asks, and claims nothing.
    let blank = join(&registry, game, account("  ", bob)).await;
    assert_eq!(blank.unwrap_err(), ClaimError::BadName);
    assert_eq!(
        states(&registry, game).await,
        [
            SeatState::Issued,
            SeatState::Claimed,
            SeatState::Open,
            SeatState::Open
        ],
        "one seat for Ann, however often she asks"
    );
    assert_eq!(names(&registry, game).await[1], "Ann");

    // Another account takes the next seat, and the registry finds each by account.
    let second = join(&registry, game, account("Bob", bob)).await.unwrap();
    assert_eq!(second.seat, seat(2));
    assert_eq!(registry.authenticate_user(game, ann).unwrap().seat, seat(1));
    assert_eq!(registry.authenticate_user(game, bob).unwrap().seat, seat(2));
    assert!(registry.authenticate_user(game, host).is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn racing_claims_never_give_a_seat_twice() {
    let registry = registry();
    let created = lobby(&registry, 12, Visibility::Public).await;
    let game = created.id;

    let racers: Vec<_> = (0..40)
        .map(|n| {
            let registry = registry.clone();
            tokio::spawn(async move { join(&registry, game, guest(&format!("Racer {n}"))).await })
        })
        .collect();
    let mut won = Vec::new();
    let mut lost = 0;
    for racer in racers {
        match racer.await.unwrap() {
            Ok(seated) => won.push(seated),
            Err(ClaimError::Full) => lost += 1,
            Err(other) => panic!("unexpected {other}"),
        }
    }

    // Eleven open seats, eleven winners, every seat once, every token its own.
    assert_eq!((won.len(), lost), (11, 29));
    let seats: HashSet<usize> = won.iter().map(|s| s.seat.index()).collect();
    assert_eq!(seats, (1..12).collect::<HashSet<_>>());
    let tokens: HashSet<String> = won
        .iter()
        .map(|s| s.token.as_ref().unwrap().expose().to_owned())
        .collect();
    assert_eq!(tokens.len(), 11);
    for seated in &won {
        let token = seated.token.as_ref().unwrap();
        let access = registry.authenticate(game, token.expose()).unwrap();
        assert_eq!(access.seat, seated.seat);
    }
    assert!(
        states(&registry, game)
            .await
            .iter()
            .all(|state| *state != SeatState::Open)
    );
    // The names at the table are the names the winners gave.
    let at_table = names(&registry, game).await;
    for seated in &won {
        assert!(at_table[seated.seat.index()].starts_with("Racer "));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn racing_claims_by_one_account_take_one_seat_and_make_one_token() {
    let registry = registry();
    let created = lobby(&registry, 6, Visibility::Public).await;
    let game = created.id;
    let ann = UserId::random();

    let racers: Vec<_> = (0..20)
        .map(|_| {
            let registry = registry.clone();
            tokio::spawn(async move { join(&registry, game, account("Ann", ann)).await })
        })
        .collect();
    let mut answers = Vec::new();
    for racer in racers {
        answers.push(racer.await.unwrap().unwrap());
    }
    assert_eq!(answers.iter().filter(|s| s.token.is_some()).count(), 1);
    let seats: HashSet<usize> = answers.iter().map(|s| s.seat.index()).collect();
    assert_eq!(seats.len(), 1);
    let claimed = states(&registry, game)
        .await
        .iter()
        .filter(|state| **state == SeatState::Claimed)
        .count();
    assert_eq!(claimed, 1);
}

#[rstest]
#[case::listed_public(Visibility::Public, Entrance::Listing, true)]
#[case::coded_public(Visibility::Public, Entrance::Code, true)]
#[case::coded_invite(Visibility::Invite, Entrance::Code, true)]
#[case::listed_invite(Visibility::Invite, Entrance::Listing, false)]
#[tokio::test]
async fn the_entrance_decides_which_games_a_visitor_can_reach(
    #[case] visibility: Visibility,
    #[case] entrance: Entrance,
    #[case] reaches: bool,
) {
    let registry = registry();
    let created = lobby(&registry, 2, visibility).await;
    let answer = registry.claim(created.id, entrance, guest("Bo")).await;
    if reaches {
        assert_eq!(answer.unwrap().seat, seat(1));
    } else {
        // The same answer as for a game that does not exist.
        assert_eq!(answer.unwrap_err(), ClaimError::NotFound);
        assert_eq!(states(&registry, created.id).await[1], SeatState::Open);
    }
}

#[tokio::test]
async fn a_game_that_does_not_exist_cannot_be_claimed() {
    let registry = registry();
    let nowhere = GameId(uuid::Uuid::from_u128(9));
    for entrance in [Entrance::Listing, Entrance::Code] {
        let answer = registry.claim(nowhere, entrance, guest("Bo")).await;
        assert_eq!(answer.unwrap_err(), ClaimError::NotFound);
    }
}

#[tokio::test]
async fn a_claim_is_told_to_the_table_and_a_release_too() {
    let registry = registry();
    let created = lobby(&registry, 3, Visibility::Public).await;
    let handle = registry.handle(created.id).unwrap();
    let mut host = host_of(&handle).await;
    host.say(&handle, ClientMessage::Ready).await;
    assert_eq!(
        host.next().await,
        roster(&[
            ("Player 0", true, true),
            ("", false, false),
            ("", false, false)
        ])
    );

    let bo = join(&registry, created.id, guest("Bo")).await.unwrap();
    assert_eq!(
        host.next().await,
        roster(&[
            ("Player 0", true, true),
            ("Bo", false, false),
            ("", false, false)
        ]),
        "the table hears of the new name, and of nothing else"
    );
    settle(&handle).await;
    silent(&host.drain());

    // Bo connects with the token, is ready, and the host hears.
    let access = registry
        .authenticate(created.id, bo.token.as_ref().unwrap().expose())
        .unwrap();
    let mut bo_client = Client::join(&access.handle, 1).await;
    bo_client.say(&handle, ClientMessage::Ready).await;
    settle(&handle).await;
    host.drain();

    // Bo leaves: the host sees the seat open again, nobody ready but the host, and Bo's queue ends.
    registry
        .release(created.id, seat(1), seat(1))
        .await
        .unwrap();
    assert_eq!(
        host.next().await,
        roster(&[
            ("Player 0", true, true),
            ("", false, false),
            ("", false, false)
        ])
    );
    settle(&handle).await;
    silent(&host.drain());
    bo_client.drain();
    assert!(
        bo_client.inbox.recv().await.is_none(),
        "the released seat's connection is closed"
    );
}

#[tokio::test]
async fn a_released_seat_loses_its_ready_and_its_token_and_can_be_claimed_by_someone_else() {
    let registry = registry();
    let created = lobby(&registry, 2, Visibility::Public).await;
    let game = created.id;
    let handle = registry.handle(game).unwrap();

    let bo = join(&registry, game, guest("Bo")).await.unwrap();
    let old = bo.token.unwrap();
    handle.send(seat(1), ClientMessage::Ready).await.unwrap();
    assert_eq!(handle.status().await.unwrap().ready, [seat(1)]);

    registry.release(game, seat(1), HOST).await.unwrap();
    assert!(registry.authenticate(game, old.expose()).is_none());
    assert_eq!(states(&registry, game).await[1], SeatState::Open);
    assert_eq!(names(&registry, game).await[1], "");
    assert_eq!(handle.status().await.unwrap().ready, Vec::<PlayerId>::new());

    let cy = join(&registry, game, guest("Cy")).await.unwrap();
    assert_eq!(cy.seat, seat(1));
    let fresh = cy.token.unwrap();
    assert_ne!(fresh.expose(), old.expose());
    assert!(registry.authenticate(game, old.expose()).is_none());
    assert_eq!(
        registry.authenticate(game, fresh.expose()).unwrap().seat,
        seat(1)
    );
    // Cy does not inherit Bo's Ready.
    assert_eq!(handle.status().await.unwrap().ready, Vec::<PlayerId>::new());
}

#[tokio::test]
async fn releasing_frees_the_account_for_a_seat_of_the_game() {
    let registry = registry();
    let created = lobby(&registry, 3, Visibility::Public).await;
    let (game, ann) = (created.id, UserId::random());

    let first = join(&registry, game, account("Ann", ann)).await.unwrap();
    registry
        .release(game, first.seat, first.seat)
        .await
        .unwrap();
    assert!(registry.authenticate_user(game, ann).is_none());

    let again = join(&registry, game, account("Ann", ann)).await.unwrap();
    assert_eq!(again.seat, seat(1));
    assert!(again.token.is_some(), "a new claim makes a new token");
    assert_eq!(registry.authenticate_user(game, ann).unwrap().seat, seat(1));
}

/// What a release by `by` of `target` comes to in a lobby where Bo (seat 1) and Cy (seat 2)
/// claimed their seats.
#[rstest]
#[case::the_holder_leaves(1, 1, Ok(()))]
#[case::the_host_removes_a_player(0, 2, Ok(()))]
#[case::a_player_removes_another(1, 2, Err(TableError::NotAllowed))]
#[case::a_player_removes_the_host(2, 0, Err(TableError::NotAllowed))]
#[case::the_host_cannot_leave_its_own_seat(0, 0, Err(TableError::NotClaimed))]
#[case::an_open_seat(0, 3, Err(TableError::NotClaimed))]
#[case::a_seat_the_table_does_not_have(0, 7, Err(TableError::NotClaimed))]
#[tokio::test]
async fn only_the_holder_and_the_host_release_and_only_claimed_seats(
    #[case] by: usize,
    #[case] target: usize,
    #[case] expected: Result<(), TableError>,
) {
    let registry = registry();
    let created = lobby(&registry, 4, Visibility::Public).await;
    let game = created.id;
    join(&registry, game, guest("Bo")).await.unwrap();
    join(&registry, game, guest("Cy")).await.unwrap();
    let before = states(&registry, game).await;

    let answer = registry.release(game, seat(target), seat(by)).await;
    assert_eq!(answer, expected);
    let after = states(&registry, game).await;
    if expected.is_ok() {
        assert_eq!(after[target], SeatState::Open);
        assert_eq!(names(&registry, game).await[target], "");
    } else {
        assert_eq!(after, before, "a refused release changes nothing");
    }
}

#[tokio::test]
async fn a_seat_made_with_the_game_is_not_given_back() {
    let registry = registry();
    let options = GameOptions {
        visibility: Visibility::Public,
        held: Some(2),
    };
    let created = registry
        .create_with(&requests(3), options, None)
        .await
        .unwrap();
    // The token of seat 1 went out with the game, so the host cannot take the seat back.
    let answer = registry.release(created.id, seat(1), HOST).await;
    assert_eq!(answer, Err(TableError::NotClaimed));
    assert_eq!(states(&registry, created.id).await[1], SeatState::Issued);
    let (_, token) = &created.seats[1];
    assert!(registry.authenticate(created.id, token.expose()).is_some());
}

#[tokio::test]
async fn releasing_in_a_game_that_does_not_exist_is_not_found() {
    let registry = registry();
    let nowhere = GameId(uuid::Uuid::from_u128(9));
    let answer = registry.release(nowhere, seat(1), seat(1)).await;
    assert_eq!(answer, Err(TableError::NotFound));
    let answer = registry
        .set_visibility(nowhere, HOST, Visibility::Public)
        .await;
    assert_eq!(answer, Err(TableError::NotFound));
}

#[tokio::test]
async fn once_the_game_starts_claims_releases_and_visibility_changes_are_refused() {
    let registry = registry();
    let created = lobby(&registry, 2, Visibility::Public).await;
    let game = created.id;
    let handle = registry.handle(game).unwrap();
    let bo = join(&registry, game, guest("Bo")).await.unwrap();
    let ann = UserId::random();
    // Bo and the host press Ready: the lobby is over.
    for number in 0..2 {
        handle
            .send(seat(number), ClientMessage::Ready)
            .await
            .unwrap();
    }
    assert!(handle.status().await.unwrap().started);

    for entrance in [Entrance::Code, Entrance::Listing] {
        let answer = registry.claim(game, entrance, guest("Cy")).await;
        assert_eq!(answer.unwrap_err(), ClaimError::Started);
    }
    // Even a signed-in visitor who would be handed a seat of their own is told it began.
    let answer = join(&registry, game, account("Ann", ann)).await;
    assert_eq!(answer.unwrap_err(), ClaimError::Started);
    for (by, target) in [(1, 1), (0, 1)] {
        let answer = registry.release(game, seat(target), seat(by)).await;
        assert_eq!(answer, Err(TableError::Started), "by {by}");
    }
    let answer = registry
        .set_visibility(game, HOST, Visibility::Invite)
        .await;
    assert_eq!(answer, Err(TableError::Started));

    // Nothing changed: Bo keeps the seat and the token.
    assert_eq!(states(&registry, game).await[1], SeatState::Claimed);
    let token = bo.token.unwrap();
    assert_eq!(
        registry.authenticate(game, token.expose()).unwrap().seat,
        seat(1)
    );
    assert_eq!(registry.visibility(game), Some(Visibility::Public));
}

#[rstest]
#[case::the_host_lists_the_game(Visibility::Invite, HOST, Visibility::Public, Ok(()))]
#[case::the_host_hides_it(Visibility::Public, HOST, Visibility::Invite, Ok(()))]
#[case::the_host_repeats_itself(Visibility::Public, HOST, Visibility::Public, Ok(()))]
#[case::a_player_may_not(
    Visibility::Invite,
    PlayerId(1),
    Visibility::Public,
    Err(TableError::NotAllowed)
)]
#[tokio::test]
async fn only_the_host_changes_who_may_find_the_game(
    #[case] from: Visibility,
    #[case] by: PlayerId,
    #[case] to: Visibility,
    #[case] expected: Result<(), TableError>,
) {
    let registry = registry();
    let created = lobby(&registry, 2, from).await;
    join(&registry, created.id, guest("Bo")).await.unwrap();

    let answer = registry.set_visibility(created.id, by, to).await;
    assert_eq!(answer, expected);
    let now = if expected.is_ok() { to } else { from };
    assert_eq!(registry.visibility(created.id), Some(now));
    assert_eq!(registry.describe(created.id).await.unwrap().visibility, now);
}
