//! Goal: helpers for the game browser tests: games made through the REST create call with a visibility and open seats, and the requests a visitor, a seat's holder and the host make.

use std::net::SocketAddr;

use mtg_core::ids::PlayerId;
use mtg_protocol::ClientMessage;
use mtg_server::Registry;
use serde_json::{Value, json};

use super::http::{Reply, request};

/// A game made through `POST /api/games`, with what the response showed once.
#[derive(Debug, Clone)]
pub struct Made {
    /// The game's id.
    pub id: String,
    /// The join code.
    pub code: String,
    /// The tokens of the held seats, in seat order; seat 0 is the host's.
    pub tokens: Vec<String>,
}

impl Made {
    /// The host's token.
    pub fn host(&self) -> &str {
        &self.tokens[0]
    }
}

/// The body of a create request for `seats` seats of which `held` are handed out: the host is
/// called `host`, the others `Player N`.
pub fn body(host: &str, seats: usize, held: usize, visibility: &str) -> Value {
    let seats: Vec<Value> = (0..seats)
        .map(|n| {
            let name = if n == 0 {
                host.to_owned()
            } else {
                format!("Player {n}")
            };
            json!({ "name": name, "deck": "mono-green" })
        })
        .collect();
    json!({ "format": "normal", "visibility": visibility, "held": held, "seats": seats })
}

/// Makes a game; the request must succeed.
pub async fn make(
    addr: SocketAddr,
    host: &str,
    seats: usize,
    held: usize,
    visibility: &str,
) -> Made {
    let reply = call(
        addr,
        "POST",
        "/api/games",
        Some(&body(host, seats, held, visibility).to_string()),
        &[],
    )
    .await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let json = reply.json();
    Made {
        id: json["game"].as_str().unwrap().to_owned(),
        code: json["code"].as_str().unwrap().to_owned(),
        tokens: json["seats"]
            .as_array()
            .unwrap()
            .iter()
            .map(|seat| seat["token"].as_str().unwrap().to_owned())
            .collect(),
    }
}

/// Any request: a JSON body when there is one, and the headers given.
pub async fn call(
    addr: SocketAddr,
    method: &str,
    path: &str,
    body: Option<&str>,
    headers: &[(&str, &str)],
) -> Reply {
    let mut all: Vec<(&str, &str)> = Vec::new();
    if body.is_some() {
        all.push(("Content-Type", "application/json"));
    }
    all.extend_from_slice(headers);
    request(addr, method, path, &all, body.unwrap_or("").as_bytes()).await
}

/// `GET /api/lobbies`.
pub async fn lobbies(addr: SocketAddr, headers: &[(&str, &str)]) -> Reply {
    call(addr, "GET", "/api/lobbies", None, headers).await
}

/// The ids in the answer of `GET /api/lobbies`, in order; the request must have succeeded.
pub fn listed(reply: &Reply) -> Vec<String> {
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()["games"]
        .as_array()
        .unwrap()
        .iter()
        .map(|game| game["game"].as_str().unwrap().to_owned())
        .collect()
}

/// `POST /api/games/{game}/join` with a JSON body.
pub async fn join(addr: SocketAddr, game: &str, body: &Value, headers: &[(&str, &str)]) -> Reply {
    let path = format!("/api/games/{game}/join");
    call(addr, "POST", &path, Some(&body.to_string()), headers).await
}

/// `POST /api/join` with a code and, for a guest, a name.
pub async fn join_code(addr: SocketAddr, code: &str, name: Option<&str>) -> Reply {
    let body = match name {
        Some(name) => json!({ "code": code, "name": name }),
        None => json!({ "code": code }),
    };
    call(addr, "POST", "/api/join", Some(&body.to_string()), &[]).await
}

/// The `Authorization` header value that presents a seat token.
pub fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

/// `POST path` with no body, presenting `token` as a bearer token.
pub async fn post_as(addr: SocketAddr, path: &str, token: &str) -> Reply {
    call(
        addr,
        "POST",
        path,
        None,
        &[("Authorization", &bearer(token))],
    )
    .await
}

/// Every seat of `game` presses Ready, so the game starts.
pub async fn start(registry: &Registry, game: &str, seats: usize) {
    let handle = registry.handle(game.parse().unwrap()).unwrap();
    for seat in 0..seats {
        handle
            .send(PlayerId::from_index(seat), ClientMessage::Ready)
            .await
            .unwrap();
    }
    // The last Ready starts the game; wait until the actor has dealt with it.
    assert!(handle.status().await.unwrap().started);
}
