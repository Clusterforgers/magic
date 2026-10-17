//! Error types returned by the engine.

use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;
use thiserror::Error;

use crate::format::DeckError;
use crate::mana::ManaSource;

/// Anything that can go wrong while driving a game.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EngineError {
    /// The game is over, so no answer is expected.
    #[error("the game is over; no decision is pending")]
    NoDecision,
    /// The seat does not exist or has already left the game.
    #[error("{0:?} is not in the game")]
    NotInGame(PlayerId),
    /// The game setup is not playable.
    #[error("invalid game setup: {0}")]
    Setup(#[from] SetupError),
    /// The answer is not one of the offered options; state was not changed.
    #[error("illegal answer: {0}")]
    Illegal(#[from] IllegalAnswer),
    /// An event does not apply to the state it was folded onto (a bug or a corrupt log).
    #[error("event does not apply: {0}")]
    InvalidEvent(String),
    /// The engine looped without reaching a decision or the end of the game.
    #[error("no progress after {0} internal steps")]
    Stalled(usize),
}

/// Why a [`GameSetup`](crate::setup::GameSetup) was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SetupError {
    /// Fewer than two players.
    #[error("a game needs at least two players, got {0}")]
    TooFewPlayers(usize),
    /// A player's deck failed the format's validation.
    #[error("player {player:?} has an invalid deck: {source}")]
    InvalidDeck {
        /// Owner of the deck.
        player: PlayerId,
        /// The validation failure.
        source: DeckError,
    },
    /// The requested starting turn or active player does not exist.
    #[error("invalid starting point: {0}")]
    InvalidStart(&'static str),
}

/// Why a mana payment was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PaymentError {
    /// A tap named a source that is not an available mana ability.
    #[error("{0:?} is not an available mana source")]
    UnknownSource(ManaSource),
    /// The same source was tapped twice.
    #[error("{0:?} was tapped twice")]
    DuplicateSource(ManaSource),
    /// More floating mana was spent than is in the pool.
    #[error("not enough {0:?} mana in the pool")]
    PoolShortfall(ManaType),
    /// The payment produces more or less mana than the cost.
    #[error("payment provides {got} mana but the cost is {needed}")]
    WrongTotal {
        /// Mana value of the cost.
        needed: u32,
        /// Mana the payment would produce.
        got: u32,
    },
    /// A colored or colorless requirement is not covered.
    #[error("the cost needs more {0:?} mana")]
    MissingMana(ManaType),
}

/// Why an answer was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IllegalAnswer {
    /// The answer is for a different kind of decision.
    #[error("expected a {expected} answer but got {got}")]
    WrongKind {
        /// Kind the pending decision needs.
        expected: &'static str,
        /// Kind that was supplied.
        got: &'static str,
    },
    /// The chosen item is not among the offered options.
    #[error("{0} is not among the legal options")]
    NotOffered(&'static str),
    /// An item was chosen more than once.
    #[error("{0} was chosen more than once")]
    Duplicate(&'static str),
    /// The wrong number of items was chosen.
    #[error("expected {expected} items but got {got}")]
    WrongCount {
        /// Required number.
        expected: usize,
        /// Supplied number.
        got: usize,
    },
    /// Combat damage was not fully divided among the blockers.
    #[error("{power} damage must be divided among the blockers but {assigned} was assigned")]
    DamageSplit {
        /// Damage the attacker must assign.
        power: u32,
        /// Damage the answer assigned.
        assigned: u32,
    },
    /// The mana payment is invalid.
    #[error("invalid payment: {0}")]
    Payment(#[from] PaymentError),
}
