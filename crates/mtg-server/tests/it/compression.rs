//! Goal: JSON answers travel gzipped to a client that accepts gzip and plain to one that does not, with the same content either way, and a game socket still opens for a client that offers gzip.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Read as _;

use flate2::read::GzDecoder;
use rstest::rstest;
use serde_json::Value;

use crate::net::{self, TestServer, bot_config};

/// The body of `reply`, unzipped when it says it is gzipped.
fn content(reply: &net::Reply) -> Value {
    if reply.header("content-encoding") == Some("gzip") {
        let mut text = String::new();
        GzDecoder::new(reply.body.as_slice())
            .read_to_string(&mut text)
            .unwrap();
        serde_json::from_str(&text).unwrap()
    } else {
        reply.json()
    }
}

#[rstest]
#[case::gzip_accepted(Some("gzip"), Some("gzip"))]
#[case::gzip_among_others(Some("br;q=1.0, gzip;q=0.8"), Some("gzip"))]
#[case::no_encoding_offered(None, None)]
#[case::only_identity(Some("identity"), None)]
#[tokio::test]
async fn a_json_answer_is_gzipped_exactly_when_the_client_accepts_gzip(
    #[case] accept: Option<&str>,
    #[case] encoding: Option<&str>,
) {
    let server = TestServer::start(bot_config()).await;
    let headers: Vec<(&str, &str)> = accept.map(|a| ("Accept-Encoding", a)).into_iter().collect();
    let reply = net::request(server.addr, "GET", "/api/demo-decks", &headers, b"").await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.header("content-encoding"), encoding);
    assert_eq!(reply.header("cache-control"), Some("no-store"));
    let plain = net::get(server.addr, "/api/demo-decks").await;
    assert_eq!(content(&reply), plain.json());
}

#[tokio::test]
async fn a_game_socket_opens_for_a_client_that_offers_gzip() {
    let server = TestServer::start(bot_config()).await;
    let (game, tokens) = net::create_game(
        server.addr,
        serde_json::json!({
            "format": "normal",
            "seats": [{ "name": "Ann", "deck": "mono-green" }, { "name": "Bob", "deck": "mono-red" }],
        }),
    )
    .await;
    let path = format!("/ws/{game}/{}", tokens[0]);
    let _socket = net::open(server.addr, &path, &[("Accept-Encoding", "gzip")]).await;
}
