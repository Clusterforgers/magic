//! Goal: a WebSocket client for tests that records every frame it receives and tells closes from resets.

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, PROTOCOL_VERSION, ServerMessage};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async_with_config};

/// No wait in a test is unbounded.
const PATIENCE: Duration = Duration::from_secs(60);

pub type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// What the next thing on the socket was.
#[derive(Debug)]
pub enum Frame {
    /// A text frame that parsed as a server message.
    Message(ServerMessage),
    /// A text frame that did not.
    Text(String),
    /// The server's close frame, with its code.
    Closed(u16),
    /// The connection ended without a close frame (reset, EOF).
    Gone,
}

pub struct WsClient {
    /// `None` once the test hung up.
    socket: Option<Socket>,
    /// Every text frame received so far, verbatim.
    pub texts: Vec<String>,
    ended: bool,
}

/// Opens a socket to `/ws/{game}/{token}`, optionally with an `Origin` header.
pub async fn try_connect(
    addr: SocketAddr,
    game: &str,
    token: &str,
    origin: Option<&str>,
) -> Result<WsClient, WsError> {
    let headers: Vec<(&str, &str)> = origin.map(|o| ("Origin", o)).into_iter().collect();
    try_open(addr, &format!("/ws/{game}/{token}"), &headers).await
}

/// Opens a socket to `path` with the given request headers (`Cookie`, `Origin`).
pub async fn try_open(
    addr: SocketAddr,
    path: &str,
    headers: &[(&str, &str)],
) -> Result<WsClient, WsError> {
    let mut request = format!("ws://{addr}{path}").into_client_request()?;
    for (name, value) in headers {
        let name = HeaderName::from_bytes(name.as_bytes()).unwrap();
        request
            .headers_mut()
            .insert(name, HeaderValue::from_str(value).unwrap());
    }
    let (socket, _) = connect_async_with_config(request, None, true).await?;
    Ok(WsClient {
        socket: Some(socket),
        texts: Vec::new(),
        ended: false,
    })
}

/// Opens a socket to `path` that must be accepted.
pub async fn open(addr: SocketAddr, path: &str, headers: &[(&str, &str)]) -> WsClient {
    match try_open(addr, path, headers).await {
        Ok(client) => client,
        Err(error) => panic!("the socket at {path} was refused: {error}"),
    }
}

/// Tries to open a socket at `path` that must be refused with an HTTP answer; returns its status
/// and body.
pub async fn refused_at(addr: SocketAddr, path: &str, headers: &[(&str, &str)]) -> (u16, String) {
    match try_open(addr, path, headers).await {
        Err(WsError::Http(response)) => {
            let body = response.body().clone().unwrap_or_default();
            (
                response.status().as_u16(),
                String::from_utf8_lossy(&body).into_owned(),
            )
        }
        Err(other) => panic!("expected an HTTP refusal, got {other}"),
        Ok(_) => panic!("the socket at {path} was accepted"),
    }
}

/// Opens a socket that must be accepted.
pub async fn connect(addr: SocketAddr, game: &str, token: &str) -> WsClient {
    match try_connect(addr, game, token, None).await {
        Ok(client) => client,
        Err(error) => panic!("the socket was refused: {error}"),
    }
}

/// Tries to open a socket that must be refused with an HTTP answer; returns its status and body.
pub async fn refused(
    addr: SocketAddr,
    game: &str,
    token: &str,
    origin: Option<&str>,
) -> (u16, String) {
    match try_connect(addr, game, token, origin).await {
        Err(WsError::Http(response)) => {
            let body = response.body().clone().unwrap_or_default();
            (
                response.status().as_u16(),
                String::from_utf8_lossy(&body).into_owned(),
            )
        }
        Err(other) => panic!("expected an HTTP refusal, got {other}"),
        Ok(_) => panic!("the socket was accepted"),
    }
}

impl WsClient {
    pub async fn send(&mut self, message: &ClientMessage) {
        self.send_text(serde_json::to_string(message).unwrap())
            .await;
    }

    pub async fn send_text(&mut self, text: impl Into<String>) {
        // A peer that already hung up is not an error here: the test looks at what comes back.
        if let Some(socket) = &mut self.socket {
            let _ = socket.send(Message::text(text.into())).await;
        }
    }

    pub async fn send_binary(&mut self, bytes: Vec<u8>) {
        if let Some(socket) = &mut self.socket {
            let _ = socket.send(Message::binary(bytes)).await;
        }
    }

    /// Sends `Join` for `game` at the current protocol version.
    pub async fn join(&mut self, game: &str) {
        self.send(&ClientMessage::Join {
            protocol_version: PROTOCOL_VERSION,
            game: game.to_owned(),
            name: "test".to_owned(),
        })
        .await;
    }

    /// Waits for the next text or close; pings and pongs are skipped.
    pub async fn next(&mut self) -> Frame {
        loop {
            let Some(socket) = self.socket.as_mut().filter(|_| !self.ended) else {
                return Frame::Gone;
            };
            let next = tokio::time::timeout(PATIENCE, socket.next())
                .await
                .expect("nothing arrived on the socket");
            match next {
                Some(Ok(Message::Text(text))) => {
                    let text = text.as_str().to_owned();
                    self.texts.push(text.clone());
                    return match serde_json::from_str(&text) {
                        Ok(message) => Frame::Message(message),
                        Err(_) => Frame::Text(text),
                    };
                }
                Some(Ok(Message::Close(frame))) => {
                    self.ended = true;
                    let code = frame.map_or(1005, |CloseFrame { code, .. }| u16::from(code));
                    // One more poll sends our half of the closing handshake, as a real client would.
                    let _ = tokio::time::timeout(Duration::from_secs(1), socket.next()).await;
                    return Frame::Closed(code);
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => {
                    self.ended = true;
                    return Frame::Gone;
                }
            }
        }
    }

    /// The next frame, which must be a server message.
    pub async fn message(&mut self) -> ServerMessage {
        match self.next().await {
            Frame::Message(message) => message,
            other => panic!("expected a server message, got {other:?}"),
        }
    }

    /// Joins `game` and returns the seat the server's `Welcome` names.
    pub async fn welcome(&mut self, game: &str) -> PlayerId {
        self.join(game).await;
        match self.message().await {
            ServerMessage::Welcome { player, .. } => player,
            other => panic!("expected Welcome, got {other:?}"),
        }
    }

    /// Reads until the server ends the connection; returns its close code, if it sent one.
    pub async fn closed(&mut self) -> Option<u16> {
        loop {
            match self.next().await {
                Frame::Closed(code) => return Some(code),
                Frame::Gone => return None,
                Frame::Message(_) | Frame::Text(_) => {}
            }
        }
    }

    /// Closes politely and waits for the server's reply.
    pub async fn close(&mut self) {
        if let Some(socket) = &mut self.socket {
            let _ = socket.send(Message::Close(None)).await;
        }
        let _ = tokio::time::timeout(Duration::from_secs(5), self.closed()).await;
    }

    /// Hangs up without a close frame.
    pub fn hang_up(&mut self) {
        self.socket = None;
    }
}
