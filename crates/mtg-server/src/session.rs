//! One game socket from the first frame to the close frame.
//!
//! A session never trusts the client with a seat: the seat comes from the token that opened the
//! socket, and every parsed message is forwarded to the actor on that seat's behalf.

use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use mtg_protocol::{ClientMessage, ErrorCode, PROTOCOL_VERSION, ServerMessage};
use tokio::sync::{mpsc, watch};
use tokio::time::{MissedTickBehavior, interval_at, timeout};

use crate::config::SocketLimits;
use crate::game_id::GameId;
use crate::limit::RateLimiter;
use crate::registry::SeatAccess;

/// Time a closing socket gets to deliver its close frame and see the peer's reply.
const CLOSE_WAIT: Duration = Duration::from_secs(2);

/// What an authenticated socket needs to run.
pub(crate) struct Session {
    pub(crate) access: SeatAccess,
    pub(crate) limits: SocketLimits,
    pub(crate) stop: watch::Receiver<bool>,
    /// Held until the socket is gone, so shutdown can wait for it.
    pub(crate) _guard: mpsc::Sender<()>,
}

/// A close frame: the code and reason the client sees.
#[derive(Debug, Clone, Copy)]
struct Goodbye {
    code: u16,
    reason: &'static str,
}

const SHUTTING_DOWN: Goodbye = Goodbye::new(1001, "server shutting down");
const GAME_ENDED: Goodbye = Goodbye::new(1001, "game closed");
const PROTOCOL_ERROR: Goodbye = Goodbye::new(1002, "protocol error");
const BAD_PAYLOAD: Goodbye = Goodbye::new(1007, "text frames must be UTF-8");
const TOO_BIG: Goodbye = Goodbye::new(1009, "message too big");
const RATE_LIMITED: Goodbye = Goodbye::new(1008, "rate limit exceeded");
const TOO_MANY_BAD: Goodbye = Goodbye::new(1008, "too many malformed messages");
const JOIN_TIMEOUT: Goodbye = Goodbye::new(1008, "no join in time");
const IDLE: Goodbye = Goodbye::new(1008, "no answer to ping");
const INTERNAL: Goodbye = Goodbye::new(1011, "internal error");
const TOO_SLOW: Goodbye = Goodbye::new(1013, "too slow; reconnect");
/// Application code: another connection took this seat, so do not reconnect on your own.
const REPLACED: Goodbye = Goodbye::new(4000, "replaced by a newer connection");

impl Goodbye {
    const fn new(code: u16, reason: &'static str) -> Goodbye {
        Goodbye { code, reason }
    }
}

/// How a session ends.
#[derive(Debug, Clone, Copy)]
enum Exit {
    /// The connection is already dead; nothing to send.
    Gone,
    /// The peer sent a close frame; finish the handshake.
    PeerClosed,
    /// Send this close frame.
    Say(Goodbye),
}

/// Runs a socket until it ends. The seat's queue is dropped on the way out, never `leave`d: a
/// replaced socket must not close the connection that replaced it.
pub(crate) async fn run(mut socket: WebSocket, mut session: Session) {
    let exit = match handshake(&mut socket, &mut session).await {
        Ok(queue) => {
            tracing::debug!(game = %session.access.game, seat = session.access.seat.get(), "socket joined");
            pump(&mut socket, queue, &mut session).await
        }
        Err(exit) => exit,
    };
    match exit {
        Exit::Gone => {}
        Exit::PeerClosed => linger(&mut socket).await,
        Exit::Say(goodbye) => say_goodbye(&mut socket, goodbye).await,
    }
}

/// Waits for `Join`, checks it and connects the seat.
async fn handshake(
    socket: &mut WebSocket,
    session: &mut Session,
) -> Result<mpsc::Receiver<ServerMessage>, Exit> {
    let limits = session.limits;
    let first = tokio::select! {
        biased;
        () = stopped(&mut session.stop) => return Err(Exit::Say(SHUTTING_DOWN)),
        first = timeout(limits.join_timeout, first_message(socket)) => first,
    };
    let text = match first {
        Err(_elapsed) => return Err(Exit::Say(JOIN_TIMEOUT)),
        Ok(Err(exit)) => return Err(exit),
        Ok(Ok(text)) => text,
    };
    let join = text.and_then(|text| serde_json::from_str::<ClientMessage>(&text).ok());
    let Some(ClientMessage::Join {
        protocol_version,
        game,
        ..
    }) = join
    else {
        return Err(refuse(
            socket,
            &limits,
            ErrorCode::BadMessage,
            "the first message must be Join",
        )
        .await);
    };
    if protocol_version != PROTOCOL_VERSION {
        let message = format!("this server speaks protocol version {PROTOCOL_VERSION}");
        return Err(refuse(socket, &limits, ErrorCode::UnsupportedVersion, message).await);
    }
    if game.parse::<GameId>().ok() != Some(session.access.game) {
        let message = "the game does not match the socket address";
        return Err(refuse(socket, &limits, ErrorCode::UnknownGame, message).await);
    }
    let seat = session.access.seat;
    session
        .access
        .handle
        .join(seat)
        .await
        .map_err(|_| Exit::Say(GAME_ENDED))
}

/// The first text frame (`None` for a binary one), skipping pings and pongs.
async fn first_message(socket: &mut WebSocket) -> Result<Option<String>, Exit> {
    loop {
        match socket.recv().await {
            Some(Ok(Message::Text(text))) => return Ok(Some(text.as_str().to_owned())),
            Some(Ok(Message::Binary(_))) => return Ok(None),
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
            Some(Ok(Message::Close(_))) => return Err(Exit::PeerClosed),
            Some(Err(error)) => return Err(Exit::Say(read_failure(&error))),
            None => return Err(Exit::Gone),
        }
    }
}

/// Pumps the seat's queue to the socket and the socket's messages to the actor.
async fn pump(
    socket: &mut WebSocket,
    mut queue: mpsc::Receiver<ServerMessage>,
    session: &mut Session,
) -> Exit {
    let Session {
        access,
        limits,
        stop,
        ..
    } = session;
    let mut limiter = RateLimiter::new(limits.burst, limits.per_second, Instant::now());
    let period = limits.ping_every.max(Duration::from_millis(1));
    let mut pings = interval_at(tokio::time::Instant::now() + period, period);
    pings.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last_pong = Instant::now();
    let mut bad_in_a_row = 0u32;
    loop {
        tokio::select! {
            biased;
            () = stopped(stop) => return Exit::Say(SHUTTING_DOWN),
            _ = access.removed.changed() => return Exit::Say(GAME_ENDED),
            outgoing = queue.recv() => match outgoing {
                Some(message) => {
                    if let Err(exit) = send(socket, &message, limits.send_timeout).await {
                        return exit;
                    }
                }
                None => return Exit::Say(why_queue_closed(access).await),
            },
            incoming = socket.recv() => {
                let frame = match incoming {
                    Some(Ok(frame)) => frame,
                    Some(Err(error)) => return Exit::Say(read_failure(&error)),
                    None => return Exit::Gone,
                };
                if !limiter.allow(Instant::now()) {
                    return Exit::Say(RATE_LIMITED);
                }
                let parsed = match &frame {
                    Message::Text(text) => serde_json::from_str::<ClientMessage>(text.as_str()).ok(),
                    Message::Binary(_) => None,
                    Message::Pong(_) => {
                        last_pong = Instant::now();
                        continue;
                    }
                    // The websocket layer answers pings by itself.
                    Message::Ping(_) => continue,
                    Message::Close(_) => return Exit::PeerClosed,
                };
                if let Some(message) = parsed {
                    bad_in_a_row = 0;
                    if access.handle.send(access.seat, message).await.is_err() {
                        return Exit::Say(GAME_ENDED);
                    }
                    continue;
                }
                bad_in_a_row += 1;
                let error = ServerMessage::Error {
                    code: ErrorCode::BadMessage,
                    message: "send each message as one JSON text frame of a known type".into(),
                };
                if let Err(exit) = send(socket, &error, limits.send_timeout).await {
                    return exit;
                }
                if bad_in_a_row >= limits.max_bad_in_a_row {
                    return Exit::Say(TOO_MANY_BAD);
                }
            },
            _ = pings.tick() => {
                if last_pong.elapsed() >= limits.idle_timeout {
                    return Exit::Say(IDLE);
                }
                let ping = socket.send(Message::Ping(Bytes::new()));
                if !matches!(timeout(limits.send_timeout, ping).await, Ok(Ok(()))) {
                    return Exit::Gone;
                }
            }
        }
    }
}

/// The close code that best fits a failed read. The websocket layer reports its own limits and
/// protocol errors only as text, so this matches on it; a wrong guess only changes the code.
fn read_failure(error: &axum::Error) -> Goodbye {
    let text = error.to_string();
    if text.contains("Space limit exceeded") {
        TOO_BIG
    } else if text.contains("UTF-8") {
        BAD_PAYLOAD
    } else {
        PROTOCOL_ERROR
    }
}

/// Resolves once the server is stopping (or gone, which comes to the same).
async fn stopped(stop: &mut watch::Receiver<bool>) {
    let _ = stop.wait_for(|stopping| *stopping).await;
}

/// The actor closed this seat's queue: another socket took the seat, the client was too slow, or the game ended.
async fn why_queue_closed(access: &SeatAccess) -> Goodbye {
    match access.handle.status().await {
        Err(_) => GAME_ENDED,
        Ok(status) if status.connected.contains(&access.seat) => REPLACED,
        Ok(_) => TOO_SLOW,
    }
}

/// Sends one server message as a text frame; a client that cannot take it in time counts as gone.
async fn send(
    socket: &mut WebSocket,
    message: &ServerMessage,
    limit: Duration,
) -> Result<(), Exit> {
    let text = serde_json::to_string(message).map_err(|error| {
        tracing::error!(%error, "a server message could not be serialized");
        Exit::Say(INTERNAL)
    })?;
    match timeout(limit, socket.send(Message::text(text))).await {
        Ok(Ok(())) => Ok(()),
        _ => Err(Exit::Gone),
    }
}

/// Tells the client why its handshake failed and ends the session.
async fn refuse(
    socket: &mut WebSocket,
    limits: &SocketLimits,
    code: ErrorCode,
    message: impl Into<String>,
) -> Exit {
    let error = ServerMessage::Error {
        code,
        message: message.into(),
    };
    match send(socket, &error, limits.send_timeout).await {
        Ok(()) => Exit::Say(PROTOCOL_ERROR),
        Err(exit) => exit,
    }
}

async fn say_goodbye(socket: &mut WebSocket, goodbye: Goodbye) {
    let frame = CloseFrame {
        code: goodbye.code,
        reason: goodbye.reason.into(),
    };
    let sent = timeout(CLOSE_WAIT, socket.send(Message::Close(Some(frame)))).await;
    if matches!(sent, Ok(Ok(()))) {
        linger(socket).await;
    }
}

/// Reads until the peer completes the close handshake (which also flushes our side), for a short while.
async fn linger(socket: &mut WebSocket) {
    let drain = async { while let Some(Ok(_)) = socket.recv().await {} };
    let _ = timeout(CLOSE_WAIT, drain).await;
}
