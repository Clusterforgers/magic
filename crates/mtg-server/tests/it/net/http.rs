//! Goal: a tiny HTTP/1.1 client over a raw TCP stream, so tests control every byte of a request, hostile ones included.

use std::fmt::Write as _;
use std::net::SocketAddr;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// A parsed response.
#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    /// The first header called `name`, case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Every header called `name`, case-insensitively, in the order they arrived.
    pub fn headers_named(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect()
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// The body as JSON; panics (failing the test) if it is not.
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("not JSON ({e}): {:?}", self.text()))
    }

    /// The `error` code of a JSON error body.
    pub fn error_code(&self) -> String {
        self.json()["error"].as_str().unwrap_or_default().to_owned()
    }
}

/// Writes `bytes` to the server, reads the whole answer and parses it. Write failures are ignored
/// because a server may answer and hang up before it has read everything.
pub async fn send_raw(addr: SocketAddr, bytes: &[u8]) -> Reply {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let _ = stream.write_all(bytes).await;
    let mut raw = Vec::new();
    let mut chunk = [0u8; 8192];
    let read = async {
        loop {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => raw.extend_from_slice(&chunk[..n]),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(10), read)
        .await
        .expect("the server kept the connection open");
    parse(&raw)
}

/// Sends a request with the given headers and body (a `Content-Length` is added when there is a body).
pub async fn request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Reply {
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    for (name, value) in headers {
        write!(head, "{name}: {value}\r\n").unwrap();
    }
    if !body.is_empty() {
        write!(head, "Content-Length: {}\r\n", body.len()).unwrap();
    }
    head.push_str("\r\n");
    let mut bytes = head.into_bytes();
    bytes.extend_from_slice(body);
    send_raw(addr, &bytes).await
}

pub async fn get(addr: SocketAddr, path: &str) -> Reply {
    request(addr, "GET", path, &[], b"").await
}

/// POSTs `body` as `application/json`.
pub async fn post_json(addr: SocketAddr, path: &str, body: &str) -> Reply {
    let headers = [("Content-Type", "application/json")];
    request(addr, "POST", path, &headers, body.as_bytes()).await
}

/// Creates a game and returns its id and the seat tokens; fails the test on anything but 201.
pub async fn create_game(addr: SocketAddr, body: Value) -> (String, Vec<String>) {
    let reply = post_json(addr, "/api/games", &body.to_string()).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let json = reply.json();
    let game = json["game"].as_str().unwrap().to_owned();
    let tokens = json["seats"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, seat)| {
            assert_eq!(seat["seat"], i);
            seat["token"].as_str().unwrap().to_owned()
        })
        .collect();
    (game, tokens)
}

fn parse(raw: &[u8]) -> Reply {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("no complete response: {:?}", String::from_utf8_lossy(raw)));
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .expect("status line");
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect();
    let body = &raw[split + 4..];
    let chunked = headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("transfer-encoding") && v.contains("chunked"));
    Reply {
        status,
        headers,
        body: if chunked {
            dechunk(body)
        } else {
            body.to_vec()
        },
    }
}

fn dechunk(mut rest: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(eol) = rest.windows(2).position(|w| w == b"\r\n") {
        let size =
            usize::from_str_radix(String::from_utf8_lossy(&rest[..eol]).trim(), 16).unwrap_or(0);
        rest = &rest[eol + 2..];
        if size == 0 || rest.len() < size {
            break;
        }
        out.extend_from_slice(&rest[..size]);
        rest = rest.get(size + 2..).unwrap_or_default();
    }
    out
}
