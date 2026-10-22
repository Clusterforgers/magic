//! Messages exchanged over the game socket.
//!
//! Both enums are internally tagged with `type`, so each message is one flat JSON object.

use mtg_core::ids::PlayerId;
use mtg_engine::forecast::CombatForecast;
use mtg_engine::state::GameResult;
use mtg_engine::turn::Step;
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

/// One seat of the lobby, as [`ServerMessage::Lobby`] lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LobbySeat {
    /// The seat.
    pub seat: PlayerId,
    /// The name at the seat; empty for an open seat that nobody holds yet.
    pub name: String,
    /// Whether a client is connected to the seat right now.
    pub connected: bool,
    /// Whether the seat has pressed Ready and not taken it back. It stays set when the client goes away.
    pub ready: bool,
    /// The deck the seat plays, as the table may know it. Left out while the seat has none, which
    /// includes an open seat: the seat cannot press Ready until its holder chooses one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub deck: Option<SeatDeck>,
}

/// What the whole table is told of a seat's deck: its name and format and how many of its cards the
/// game cannot run on its own, never the name of a card (`docs/deck-check.md`, Decisions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SeatDeck {
    /// The deck's name, as its owner wrote it or as a built-in deck is called.
    pub name: String,
    /// The deck's format key, such as `modern` or `commander`; `normal` for a built-in deck.
    pub format: String,
    /// Copies of the main deck's cards that are not verified, which the table resolves by hand in
    /// part or in whole. Zero for a deck the game plays entirely.
    pub manual_cards: u32,
}

/// Whether the table plays with manual cards: some seat's deck holds cards the game does not run whole, which their
/// controllers do by hand with manual actions (`docs/engine.md`, Manual actions). A seat without a deck says nothing.
pub fn manual_play(seats: &[LobbySeat]) -> bool {
    seats
        .iter()
        .filter_map(|seat| seat.deck.as_ref())
        .any(|deck| deck.manual_cards > 0)
}

/// How long a seat asks the server to pass priority for it (see [`ClientMessage::Yield`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub enum YieldUntil {
    /// Until the stack is empty.
    Resolve,
    /// Until the turn ends.
    EndOfTurn,
    /// Until the seat's own next turn begins.
    MyTurn,
}

/// Whose turns a stop applies to (see [`ClientMessage::SetStop`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
pub enum StopSide {
    /// The turns of the seat that holds the stop.
    Own,
    /// The turn of any opponent.
    Opponent,
}

/// A step at which a seat asks to be given priority (see [`ClientMessage::SetStop`]).
///
/// Ordered own side first, then by turn order, which is the order the server lists them in. A seat holds one
/// stop per side and step, so `keep`, the last field, never decides where a stop is listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
pub struct Stop {
    /// Whose turns the stop is for.
    pub side: StopSide,
    /// The step; only a step where a player gets priority can hold a stop, so never untap or cleanup.
    pub step: Step,
    /// How long the stop lasts: false until that step has passed once, true until it is cleared.
    pub keep: bool,
}

/// How much of the game a seat plays by hand (see [`ClientMessage::FullControl`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub enum FullControlMode {
    /// The server passes for the seat by its usual rules.
    #[default]
    Off,
    /// The seat is asked at every step until it has passed once, then the mode returns to `Off`.
    NextWindow,
    /// The seat is asked at every step until it turns the mode off.
    Held,
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
    /// The player takes [`ClientMessage::Ready`] back, until the game starts; harmless after that.
    NotReady,
    /// Answers the pending decision with this id.
    Answer {
        /// The decision being answered.
        decision: DecisionId,
        /// The chosen answer, built from the decision's options.
        answer: Answer,
    },
    /// Asks what combat damage would do if `answer` were given to the pending decision `decision`, without giving it.
    ///
    /// Advisory: the server replies with exactly one [`ServerMessage::Forecast`], never an error, and logs nothing.
    Preview {
        /// The decision the answer is for.
        decision: DecisionId,
        /// The answer to try, built from the decision's options like a real one.
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
    /// Asks the server to pass priority for this seat until `until`; [`ServerMessage::Yielding`] says when it starts or ends.
    ///
    /// While it lasts the server answers each `Priority` decision of the seat with a pass and never sends it; the
    /// pending one is passed at once. It ends, and decisions come as usual, when an opponent puts a new object on
    /// the stack, at `until`, when the seat leaves the game and on `CancelYield`; what the seat could do about it
    /// never matters. Other decisions (attackers, blockers, a discard, a payment) are always sent and leave it as
    /// it is. A new `Yield` replaces the old one. Kept in memory per seat: a resumed game starts with none.
    Yield {
        /// How long to pass for.
        until: YieldUntil,
    },
    /// Ends this seat's yield, if it has one.
    CancelYield,
    /// Sets or clears one of this seat's stops: a step at which the seat asks to be given priority.
    ///
    /// Only a step where a player gets priority can hold a stop. `Untap` and `Cleanup` are refused, set or
    /// cleared alike, with `Error{BadMessage}`: nothing changes and no resync is needed. `keep` says how long
    /// a stop that is set lasts: false until that step has passed once, true until it is cleared; setting a
    /// stop that is already set replaces its `keep` (a lasting stop becomes a one-turn one and back), and
    /// clearing ignores it. A request that changes the seat's stops, `keep` included, is answered with
    /// [`ServerMessage::Stops`], one that changes nothing with silence. A seat that is out of the game, or a
    /// game that is over, gets `Error{BadMessage}` too. The server holds the stops per seat in memory, from the
    /// lobby on, and only stores and reports them for now: no seat is asked at a stop yet.
    SetStop {
        /// Whose turns the stop is for.
        side: StopSide,
        /// The step; refused unless a player gets priority in it.
        step: Step,
        /// True to set the stop, false to clear it.
        on: bool,
        /// With `on`: false drops the stop once that step has passed, true keeps it until it is cleared.
        keep: bool,
    },
    /// Sets how much of the game this seat plays by hand; [`ServerMessage::FullControl`] says what the server holds.
    ///
    /// Kept per seat in memory like the stops, and only stored and reported for now: no seat is asked at every
    /// step yet. Answered like [`ClientMessage::SetStop`]: [`ServerMessage::FullControl`] when the mode changes,
    /// silence when it does not, `Error{BadMessage}` from a seat that is out of the game.
    FullControl {
        /// The new mode.
        mode: FullControlMode,
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
        /// The yield the server holds for this seat, if any (see [`ClientMessage::Yield`]).
        yielding: Option<YieldUntil>,
        /// The stops the server holds for this seat, in the order of [`ServerMessage::Stops`].
        stops: Vec<Stop>,
        /// The Full Control mode the server holds for this seat (see [`ClientMessage::FullControl`]).
        full_control: FullControlMode,
    },
    /// Who is at the table before the game starts.
    ///
    /// Sent to every connected seat right after its `Welcome` and again whenever a seat joins, leaves or
    /// disconnects, presses [`ClientMessage::Ready`] or takes it back. Never sent once the game runs: the
    /// opening events take over, and a client that joins a running game gets a snapshot instead.
    Lobby {
        /// Every seat in seat order.
        seats: Vec<LobbySeat>,
        /// How many seats still have to press Ready before the game starts.
        needed: u32,
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
    /// The reply to [`ClientMessage::Preview`]: what combat damage would do for the answer tried.
    Forecast {
        /// The decision the preview named, echoed.
        decision: DecisionId,
        /// The engine's forecast, or null when there is none to give: the decision is not pending for this seat, the
        /// answer is not a legal attack or block, or this seat is previewing faster than the server allows.
        forecast: Option<CombatForecast>,
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
    /// This seat's yield changed (see [`ClientMessage::Yield`]).
    ///
    /// Sent whenever it starts, is replaced or ends, after the events of the step that changed it and before
    /// the decision that follows. Ending covers every cause, so a client never works out when a yield stopped.
    Yielding {
        /// The yield now in force, or `None` when the seat is not yielding.
        until: Option<YieldUntil>,
    },
    /// The stops this seat holds changed (see [`ClientMessage::SetStop`]).
    ///
    /// Sent whenever the set the seat can see changes, whatever the cause, after the events of the step that
    /// changed it and before the decision that follows, like [`ServerMessage::Yielding`]. It lists every stop
    /// the seat holds with its `keep`, once each, own side first and each side in turn order, so the list
    /// replaces the client's. Replacing only the `keep` of a stop changes the list, so it is told too.
    Stops {
        /// The stops now held; empty when there are none.
        stops: Vec<Stop>,
    },
    /// This seat's Full Control mode changed (see [`ClientMessage::FullControl`]).
    ///
    /// Sent whenever the mode changes, whatever the cause, in the same place as [`ServerMessage::Stops`].
    FullControl {
        /// The mode now in force.
        mode: FullControlMode,
    },
}
