//! Goal: the game browser lists exactly the public games that are still in their lobby, newest first and at most fifty, with the host, the format, how many seats are filled and when the game was made, follows claims, releases, the visibility and the start, never holds a token, a join code or a seat link, and may be asked for thirty times a minute from one address.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::fmt::Write;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use common::seat;
use mtg_protocol::ClientMessage;
use mtg_server::registry::{
    Created, Entrance, GameOptions, HOST, ListedGame, MAX_LISTED_LOBBIES, SeatClaim, SeatRequest,
};
use mtg_server::{FormatKind, GameId, Registry, ServerConfig, Visibility, demo};

fn requests(host: &str, seats: usize) -> Vec<SeatRequest> {
    (0..seats)
        .map(|n| SeatRequest {
            name: if n == 0 {
                host.to_owned()
            } else {
                format!("Player {n}")
            },
            deck: &demo::all()[n % demo::all().len()],
        })
        .collect()
}

fn registry() -> Registry {
    Registry::new(&ServerConfig::default())
}

/// Creates a game of `seats` seats with `held` of them held, then waits a moment so that the next
/// game is strictly newer.
async fn game(
    registry: &Registry,
    host: &str,
    seats: usize,
    held: usize,
    visibility: Visibility,
) -> Created {
    let options = GameOptions {
        visibility,
        held: Some(held),
        ..GameOptions::default()
    };
    let created = registry
        .create_with(&requests(host, seats), options, None)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(2)).await;
    created
}

fn ids(listed: &[ListedGame]) -> Vec<GameId> {
    listed.iter().map(|game| game.game).collect()
}

async fn claim(registry: &Registry, game: GameId, name: &str) {
    let claim = SeatClaim {
        name: name.to_owned(),
        user: None,
    };
    registry.claim(game, Entrance::Code, claim).await.unwrap();
}

#[tokio::test]
async fn only_public_games_in_their_lobby_are_listed_newest_first() {
    let registry = registry();
    let invite_lobby = game(&registry, "Ann", 3, 1, Visibility::Invite).await;
    let invite_all_held = game(&registry, "Ann", 2, 2, Visibility::Invite).await;
    let first = game(&registry, "Bo", 3, 1, Visibility::Public).await;
    let second = game(&registry, "Cy", 2, 1, Visibility::Public).await;
    let third = game(&registry, "Di", 4, 2, Visibility::Public).await;
    assert_eq!(
        ids(&registry.lobbies().await),
        [third.id, second.id, first.id]
    );
    for hidden in [invite_lobby.id, invite_all_held.id] {
        assert!(!ids(&registry.lobbies().await).contains(&hidden));
    }

    // A public game that has started leaves the list.
    let handle = registry.handle(second.id).unwrap();
    claim(&registry, second.id, "Ed").await;
    for number in 0..2 {
        handle
            .send(seat(number), ClientMessage::Ready)
            .await
            .unwrap();
    }
    assert!(handle.status().await.unwrap().started);
    assert_eq!(ids(&registry.lobbies().await), [third.id, first.id]);
}

#[tokio::test]
async fn nothing_is_listed_without_a_public_lobby() {
    let registry = registry();
    assert_eq!(registry.lobbies().await, []);
    game(&registry, "Ann", 2, 1, Visibility::Invite).await;
    assert_eq!(registry.lobbies().await, []);
}

#[tokio::test]
async fn a_listed_game_says_who_hosts_it_which_format_how_full_it_is_and_how_old() {
    let registry = registry();
    let created = game(&registry, "Ann", 4, 1, Visibility::Public).await;
    let listed = registry.lobbies().await;
    assert_eq!(listed.len(), 1);
    let one = &listed[0];
    assert_eq!(one.game, created.id);
    assert_eq!(one.host, "Ann");
    assert_eq!(one.format, FormatKind::Normal);
    assert_eq!((one.filled, one.total), (1, 4));
    assert!(one.age < Duration::from_secs(60), "{:?}", one.age);

    // The stamp reads YYYY-MM-DDTHH:MM:SSZ.
    let shape: Vec<char> = one
        .created_at
        .chars()
        .map(|c| if c.is_ascii_digit() { '9' } else { c })
        .collect();
    assert_eq!(
        shape.iter().collect::<String>(),
        "9999-99-99T99:99:99Z",
        "{}",
        one.created_at
    );
    assert!(one.created_at.starts_with("20"));
}

#[tokio::test]
async fn claims_and_releases_move_the_count_and_a_full_lobby_stays_listed() {
    let registry = registry();
    let created = game(&registry, "Ann", 3, 1, Visibility::Public).await;
    let filled = |listed: &[ListedGame]| (listed[0].filled, listed[0].total);

    claim(&registry, created.id, "Bo").await;
    assert_eq!(filled(&registry.lobbies().await), (2, 3));
    claim(&registry, created.id, "Cy").await;
    // Full, and waiting for everybody to press Ready: the row says so and is still there.
    assert_eq!(filled(&registry.lobbies().await), (3, 3));
    registry
        .release(created.id, seat(1), seat(1))
        .await
        .unwrap();
    assert_eq!(filled(&registry.lobbies().await), (2, 3));
    registry.release(created.id, seat(2), HOST).await.unwrap();
    assert_eq!(filled(&registry.lobbies().await), (1, 3));
}

#[tokio::test]
async fn the_host_lists_and_hides_a_game() {
    let registry = registry();
    let created = game(&registry, "Ann", 2, 1, Visibility::Invite).await;
    assert_eq!(registry.lobbies().await, []);
    registry
        .set_visibility(created.id, HOST, Visibility::Public)
        .await
        .unwrap();
    assert_eq!(ids(&registry.lobbies().await), [created.id]);
    registry
        .set_visibility(created.id, HOST, Visibility::Invite)
        .await
        .unwrap();
    assert_eq!(registry.lobbies().await, []);
}

#[tokio::test]
async fn at_most_fifty_games_are_listed_the_newest() {
    let registry = registry();
    let mut made = Vec::new();
    for n in 0..(MAX_LISTED_LOBBIES + 6) {
        made.push(
            game(&registry, &format!("Host {n}"), 2, 1, Visibility::Public)
                .await
                .id,
        );
    }
    // A newer invite game does not take a place.
    game(&registry, "Quiet", 2, 1, Visibility::Invite).await;

    let listed = registry.lobbies().await;
    assert_eq!(listed.len(), MAX_LISTED_LOBBIES);
    assert_eq!(MAX_LISTED_LOBBIES, 50);
    let newest: Vec<GameId> = made
        .iter()
        .rev()
        .take(MAX_LISTED_LOBBIES)
        .copied()
        .collect();
    assert_eq!(ids(&listed), newest);
    assert_eq!(listed[0].host, format!("Host {}", MAX_LISTED_LOBBIES + 5));
}

#[tokio::test]
async fn games_that_started_do_not_use_up_the_fifty() {
    let registry = registry();
    let mut lobbies = Vec::new();
    for n in 0..4 {
        lobbies.push(
            game(&registry, &format!("Early {n}"), 2, 1, Visibility::Public)
                .await
                .id,
        );
    }
    // Fifty newer games that have all started, so only lobbies count towards the cap.
    for n in 0..MAX_LISTED_LOBBIES {
        let started = game(&registry, &format!("Late {n}"), 2, 2, Visibility::Public).await;
        let handle = registry.handle(started.id).unwrap();
        for number in 0..2 {
            handle
                .send(seat(number), ClientMessage::Ready)
                .await
                .unwrap();
        }
    }
    let listed = registry.lobbies().await;
    let expected: Vec<GameId> = lobbies.iter().rev().copied().collect();
    assert_eq!(ids(&listed), expected);
}

#[tokio::test]
async fn a_listing_holds_no_token_no_code_and_no_digest() {
    let registry = registry();
    let created = game(&registry, "Ann", 3, 2, Visibility::Public).await;
    let bo = registry
        .claim(
            created.id,
            Entrance::Listing,
            SeatClaim {
                name: "Bo".to_owned(),
                user: None,
            },
        )
        .await
        .unwrap();
    let mut secrets: Vec<String> = created
        .seats
        .iter()
        .map(|(_, token)| token.expose().to_owned())
        .collect();
    secrets.push(bo.token.as_ref().unwrap().expose().to_owned());
    secrets.push(created.code.expose().to_owned());
    let hex = |bytes: &[u8; 32]| {
        bytes.iter().fold(String::new(), |mut out, byte| {
            write!(out, "{byte:02x}").unwrap();
            out
        })
    };
    secrets.push(hex(created.code.digest().as_bytes()));
    for (_, token) in &created.seats {
        secrets.push(hex(token.digest().as_bytes()));
    }

    let listed = registry.lobbies().await;
    let shown = format!("{listed:?} {listed:#?}");
    for secret in &secrets {
        assert!(
            !shown.contains(secret.as_str()),
            "the listing shows {secret}"
        );
    }
    // The fields are the ones the browser needs and no others.
    let ListedGame {
        game: _,
        host: _,
        format: _,
        house_rules: _,
        manual_play: _,
        filled: _,
        total: _,
        created_at: _,
        age: _,
    } = &listed[0];
}

#[tokio::test]
async fn an_address_may_ask_for_the_list_thirty_times_a_minute() {
    let registry = registry();
    let me: IpAddr = "192.0.2.7".parse().unwrap();
    let t0 = Instant::now();
    for n in 0..30 {
        let asked = registry.admit_listing(me, t0 + Duration::from_secs(n));
        assert_eq!(asked, Ok(()), "request {n}");
    }
    // The window began with the first request and lasts a minute.
    let blocked = registry
        .admit_listing(me, t0 + Duration::from_secs(40))
        .unwrap_err();
    assert_eq!(blocked.retry_after, Duration::from_secs(20));
    assert_eq!(blocked.seconds(), 20);
    // Another address has a budget of its own, and the first one is free again a minute on.
    assert_eq!(
        registry.admit_listing("192.0.2.8".parse().unwrap(), t0),
        Ok(())
    );
    assert_eq!(
        registry.admit_listing(me, t0 + Duration::from_secs(60)),
        Ok(())
    );
}
