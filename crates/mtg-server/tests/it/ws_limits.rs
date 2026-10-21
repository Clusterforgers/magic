//! Goal: a connected socket is held to its limits: it must join in time, stay under the frame size and message rate, send no run of malformed messages and answer pings, and breaking a limit closes it without ever touching the game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, ErrorCode, ServerMessage};
use mtg_server::ServerConfig;
use mtg_server::config::SocketLimits;
use net::{Table, table_config};

/// The table config with some socket limits changed.
fn with_limits(change: impl FnOnce(&mut SocketLimits)) -> ServerConfig {
    let mut config = table_config();
    change(&mut config.socket);
    config
}

#[tokio::test]
async fn silence_after_connecting_is_closed_after_the_join_timeout() {
    let config = with_limits(|l| l.join_timeout = Duration::from_millis(300));
    let table = Table::start(config, true).await;
    let mut client = net::connect(table.addr(), &table.game, &table.tokens[0]).await;
    assert_eq!(client.closed().await, Some(1008));
    table.nobody_connected().await;
    table.assert_untouched().await;
}

#[tokio::test]
async fn frames_over_the_limit_close_the_socket_and_never_reach_the_game() {
    let table = Table::start(table_config(), true).await;
    let max = SocketLimits::default().max_frame;
    let seat = table.waiting;
    let padded = |text: &str, size: usize| format!("{text}{}", " ".repeat(size - text.len()));

    // A frame of exactly the limit is accepted: Resync padded with blanks gets its snapshot.
    let mut exact = table.player(seat).await;
    table.expect_resync(&mut exact).await;
    exact.send_text(padded(r#"{"type":"Resync"}"#, max)).await;
    assert!(matches!(
        exact.message().await,
        ServerMessage::Snapshot { .. }
    ));
    exact.close().await;

    // One byte more is too much. The message is a valid Concede, and it must not be applied.
    let concede = r#"{"type":"Concede"}"#;
    for size in [max + 1, 5 * max] {
        let mut big = table.player(seat).await;
        big.send_text(padded(concede, size)).await;
        // Usually the 1009 close frame; a peer that has already been reset sees no frame at all.
        let code = big.closed().await;
        assert!(matches!(code, Some(1009 | 1002) | None), "{code:?}");
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    table.assert_untouched().await;
}

#[tokio::test]
async fn garbage_gets_an_error_each_and_five_in_a_row_close_the_socket() {
    let table = Table::start(table_config(), true).await;
    let seat = table.waiting;
    let mut client = table.player(seat).await;
    table.expect_resync(&mut client).await;

    let junk = [
        r#"{"type":"Concede"}x"#,
        r#"{"type":"Nope"}"#,
        r#"{"type":"Answer","decision":"seven","answer":null}"#,
        "{",
        "\u{0}\u{1}",
    ];
    let is_bad_message = |message: ServerMessage| {
        matches!(
            message,
            ServerMessage::Error {
                code: ErrorCode::BadMessage,
                ..
            }
        )
    };
    // Four bad messages, then a good one: the count starts again.
    for round in 0..2 {
        for text in &junk[..4] {
            client.send_text(*text).await;
            assert!(is_bad_message(client.message().await), "{text}");
        }
        if round == 0 {
            client.send(&ClientMessage::Resync).await;
            table.expect_resync(&mut client).await;
        }
    }
    // The fifth in a row is answered and then the socket is closed.
    client.send_text(junk[4]).await;
    assert!(is_bad_message(client.message().await));
    assert_eq!(client.closed().await, Some(1008));

    table.assert_untouched().await;
    // The seat can come back.
    let mut again = table.player(seat).await;
    table.expect_resync(&mut again).await;
}

#[tokio::test]
async fn a_flood_is_cut_off_by_the_rate_limit() {
    let config = with_limits(|l| {
        l.burst = 5;
        l.per_second = 1;
    });
    let table = Table::start(config, true).await;
    let mut client = table.player(table.waiting).await;
    for _ in 0..50 {
        client.send(&ClientMessage::Resync).await;
    }
    assert_eq!(client.closed().await, Some(1008));
    table.assert_untouched().await;
}

#[tokio::test]
async fn a_client_that_answers_pings_stays_and_one_that_does_not_is_dropped() {
    let config = with_limits(|l| {
        l.ping_every = Duration::from_millis(100);
        l.idle_timeout = Duration::from_millis(500);
    });
    let table = Table::start(config, false).await;

    // Seat 0 keeps reading, which answers every ping, and is still there after many timeouts.
    let mut alive = table.player(0).await;
    let stayed = tokio::time::timeout(Duration::from_millis(2_000), alive.closed()).await;
    assert!(
        stayed.is_err(),
        "a client that answers pings was dropped: {stayed:?}"
    );
    assert_eq!(
        table.handle.status().await.unwrap().connected,
        [PlayerId::from_index(0)]
    );

    // Seat 1 joins and then never reads, so it never pongs.
    let mut deaf = net::connect(table.addr(), &table.game, &table.tokens[1]).await;
    deaf.join(&table.game).await;
    // Seat 0 keeps reading (and so ponging) while the deaf seat runs out of time.
    let stayed = tokio::time::timeout(Duration::from_millis(1_500), alive.closed()).await;
    assert!(stayed.is_err(), "seat 0 was dropped: {stayed:?}");
    let connected = table.handle.status().await.unwrap().connected;
    assert_eq!(
        connected,
        [PlayerId::from_index(0)],
        "the deaf seat is dropped"
    );
    assert_eq!(deaf.closed().await, Some(1008));
}
