//! Messages exchanged over the game socket.
//!
//! Both enums are internally tagged with `type`, so each message is one flat JSON object.

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use mtg_engine::{Answer, Decision};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::events::EventBatch;
use crate::view::StateView;

/// Version of this wire protocol; both sides state it in the handshake.
pub const PROTOCOL_VERSION: u32 = 1;

/// Identifies one pending decision so a stale answer cannot be applied to a later one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
pub struct DecisionId(pub u64);

/// What went wrong, in terms a client can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ErrorCode {
    /// The client's protocol version is not supported.
    UnsupportedVersion,
    /// The message was malformed or arrived out of order.
    BadMessage,
    /// The game does not exist or the seat cannot be taken.
    UnknownGame,
    /// The answer refers to a decision that is no longer pending.
    StaleDecision,
    /// The engine rejected the answer as illegal.
    IllegalAnswer,
    /// The server failed; the game itself is unchanged.
    Internal,
}

/// A message from a client to the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// First message: identifies the game and the protocol version the client speaks.
    Join {
        /// The client's [`PROTOCOL_VERSION`].
        protocol_version: u32,
        /// The game to join.
        game: String,
        /// Display name for chat and the log.
        name: String,
    },
    /// The player is ready for the game to start.
    Ready,
    /// Answers the pending decision with this id.
    Answer {
        /// The decision being answered.
        decision: DecisionId,
        /// The chosen answer, built from the decision's options.
        answer: Answer,
    },
    /// A chat line for the table.
    Chat {
        /// The text.
        text: String,
    },
    /// The player gives up.
    Concede,
    /// Asks for a fresh snapshot, for example after a reconnect or a gap in the event stream.
    Resync,
    /// Turns "pass for me when I cannot act" on or off for this seat.
    ///
    /// While it is on, a priority decision whose `can_act` is false is answered with a pass by the
    /// server and never sent. Kept per seat for the life of the game (a resumed game starts every seat
    /// at the server default); the next messages the seat receives show its effect.
    AutoPass {
        /// The new setting.
        enabled: bool,
    },
}

/// A message from the server to one client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// Reply to [`ClientMessage::Join`].
    Welcome {
        /// The server's [`PROTOCOL_VERSION`].
        protocol_version: u32,
        /// The game joined.
        game: String,
        /// The seat this client plays.
        player: PlayerId,
        /// Number of seats at the table.
        seats: u32,
        /// Whether the server passes for this seat when it cannot act (see [`ClientMessage::AutoPass`]).
        auto_pass: bool,
    },
    /// The client must answer; the engine's legal options are inside.
    Decision {
        /// Echo this id in the answer.
        id: DecisionId,
        /// The question with every legal option.
        decision: Decision,
        /// Milliseconds the seat has left when this is sent; absent for a decision without a clock.
        /// At the end the server answers for the seat (a pass, or no blocks) and this decision goes stale.
        /// A decision sent again after a reconnect carries the time that is left, not the time it started with.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        deadline_ms: Option<u64>,
    },
    /// New events, projected for this client.
    Events(EventBatch),
    /// The whole game as this client may see it.
    Snapshot {
        /// Number of log events the snapshot already includes; the next batch starts here.
        seq: u64,
        /// The view.
        state: Box<StateView>,
    },
    /// A chat line from a player.
    Chat {
        /// The sender.
        from: PlayerId,
        /// The text.
        text: String,
    },
    /// The last client message was rejected.
    Error {
        /// Machine-readable reason.
        code: ErrorCode,
        /// Human-readable detail.
        message: String,
    },
    /// The game is over.
    GameOver {
        /// How it ended.
        result: GameResult,
    },
}
