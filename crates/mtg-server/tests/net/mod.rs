//! Goal: helpers for the transport tests: a server on an ephemeral port, a raw HTTP client, WebSocket clients and bot players that talk to it over real sockets.
#![allow(dead_code, unused_imports, clippy::unwrap_used, clippy::expect_used)]

pub mod bot;
pub mod http;
pub mod server;
pub mod table;
pub mod ws;

pub use bot::WsBot;
pub use http::{Reply, create_game, get, post_json, request, send_raw};
pub use server::{TestServer, bot_config};
pub use table::{Table, table_config, two_seats};
pub use ws::{Frame, WsClient, connect, refused, try_connect};
