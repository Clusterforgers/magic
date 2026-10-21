//! Goal: sign accounts up and in over the real REST surface, and act as a browser that holds the session cookie it was given.

use std::net::SocketAddr;

use serde_json::{Value, json};

use super::http::{Reply, request};

/// A password that passes the rules.
pub const PASSWORD: &str = "correct horse battery";

/// The body of a register or login request.
pub fn credentials(username: &str, password: &str) -> String {
    json!({ "username": username, "password": password }).to_string()
}

/// The `name=value` part of the `Set-Cookie` header that sets `mtg_session`, if the reply has one.
pub fn session_cookie(reply: &Reply) -> Option<String> {
    reply
        .headers_named("set-cookie")
        .into_iter()
        .find(|line| line.starts_with("mtg_session="))
        .and_then(|line| line.split(';').next())
        .map(str::to_owned)
}

/// A signed-in browser: the cookie it sends and who it is.
#[derive(Debug, Clone)]
pub struct Browser {
    /// `mtg_session=<token>`, as the `Cookie` header carries it.
    pub cookie: String,
    /// The session token alone.
    pub token: String,
    /// The account's id.
    pub id: String,
    /// The username as registered.
    pub username: String,
}

impl Browser {
    fn from_reply(reply: &Reply) -> Browser {
        let cookie = session_cookie(reply)
            .unwrap_or_else(|| panic!("no session cookie in {:?}", reply.headers));
        let user = reply.json();
        Browser {
            token: cookie.trim_start_matches("mtg_session=").to_owned(),
            cookie,
            id: user["id"].as_str().expect("an id").to_owned(),
            username: user["username"].as_str().expect("a username").to_owned(),
        }
    }

    /// `GET path` with this browser's cookie.
    pub async fn get(&self, addr: SocketAddr, path: &str) -> Reply {
        request(addr, "GET", path, &[("Cookie", &self.cookie)], b"").await
    }

    /// `POST path` with a JSON body and this browser's cookie.
    pub async fn post(&self, addr: SocketAddr, path: &str, body: &str) -> Reply {
        let headers = [
            ("Content-Type", "application/json"),
            ("Cookie", self.cookie.as_str()),
        ];
        request(addr, "POST", path, &headers, body.as_bytes()).await
    }

    /// `PUT path` with a JSON body and this browser's cookie.
    pub async fn put(&self, addr: SocketAddr, path: &str, body: &str) -> Reply {
        self.send(addr, "PUT", path, body, &[]).await
    }

    /// `DELETE path` with this browser's cookie.
    pub async fn delete(&self, addr: SocketAddr, path: &str) -> Reply {
        request(addr, "DELETE", path, &[("Cookie", &self.cookie)], b"").await
    }

    /// Any request with this browser's cookie, a JSON body and the extra headers given.
    pub async fn send(
        &self,
        addr: SocketAddr,
        method: &str,
        path: &str,
        body: &str,
        extra: &[(&str, &str)],
    ) -> Reply {
        let mut headers = vec![
            ("Content-Type", "application/json"),
            ("Cookie", self.cookie.as_str()),
        ];
        headers.extend_from_slice(extra);
        request(addr, method, path, &headers, body.as_bytes()).await
    }

    /// Creates a game as this user, which must succeed; returns its id and the seat tokens.
    pub async fn create_game(&self, addr: SocketAddr, body: &Value) -> (String, Vec<String>) {
        let reply = self.post(addr, "/api/games", &body.to_string()).await;
        assert_eq!(reply.status, 201, "{}", reply.text());
        let json = reply.json();
        let tokens = json["seats"]
            .as_array()
            .unwrap()
            .iter()
            .map(|seat| seat["token"].as_str().unwrap().to_owned())
            .collect();
        (json["game"].as_str().unwrap().to_owned(), tokens)
    }

    /// `GET /api/me/games`, which must succeed; returns the `games` array.
    pub async fn games(&self, addr: SocketAddr) -> Vec<Value> {
        let reply = self.get(addr, "/api/me/games").await;
        assert_eq!(reply.status, 200, "{}", reply.text());
        reply.json()["games"].as_array().unwrap().clone()
    }
}

/// Registers an account, which must succeed, and returns the browser it signed in.
pub async fn register(addr: SocketAddr, username: &str, password: &str) -> Browser {
    let body = credentials(username, password);
    let headers = [("Content-Type", "application/json")];
    let reply = request(
        addr,
        "POST",
        "/api/auth/register",
        &headers,
        body.as_bytes(),
    )
    .await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    Browser::from_reply(&reply)
}

/// Logs in, which must succeed, and returns the new browser session.
pub async fn login(addr: SocketAddr, username: &str, password: &str) -> Browser {
    let body = credentials(username, password);
    let headers = [("Content-Type", "application/json")];
    let reply = request(addr, "POST", "/api/auth/login", &headers, body.as_bytes()).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    Browser::from_reply(&reply)
}

/// `GET /api/auth/me`, with a cookie or without.
pub async fn me(addr: SocketAddr, cookie: Option<&str>) -> Reply {
    let headers: Vec<(&str, &str)> = cookie
        .map(|cookie| ("Cookie", cookie))
        .into_iter()
        .collect();
    request(addr, "GET", "/api/auth/me", &headers, b"").await
}
