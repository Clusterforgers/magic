//! Error types returned by the engine.

use mtg_core::ids::{ObjectId, PlayerId};
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
    /// A spell or ability could not be run: it uses something the engine does not play, or its data is wrong.
    #[error("cannot resolve: {0}")]
    Resolution(String),
}

/// Why an answer could not be previewed with [`Engine::preview_forecast`](crate::engine::Engine::preview_forecast).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PreviewError {
    /// The game is over, so no decision is pending.
    #[error("the game is over; no decision is pending")]
    NoDecision,
    /// The pending decision belongs to another seat.
    #[error("the pending decision is not {0:?}'s")]
    NotYourDecision(PlayerId),
    /// The pending decision (named here) has no combat outcome to forecast.
    #[error("a {0} decision has no combat outcome to preview")]
    NotPreviewable(&'static str),
    /// The real answer would be refused for this reason, so there is nothing to forecast.
    #[error("illegal answer: {0}")]
    Illegal(#[from] IllegalAnswer),
    /// The answer declares no attacker, so there is no combat to forecast.
    #[error("no attacker would be declared, so there is no combat to forecast")]
    NoCombat,
    /// The scratch copy of the game would not take the answer's events (a bug or a corrupt state).
    #[error("the answer could not be tried on a copy of the game: {0}")]
    Engine(EngineError),
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
    /// A starting permanent is attached to itself or to a place that holds no starting permanent.
    #[error(
        "the permanent at place {index} of seat {seat} is attached to nothing it can be attached to"
    )]
    BadAttachment {
        /// The seat whose battlefield lists it.
        seat: usize,
        /// Its place there.
        index: usize,
    },
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
    /// Two abilities of one permanent were used, which would tap it twice.
    #[error("{0:?} would be tapped twice")]
    ObjectTappedTwice(ObjectId),
    /// A tap named a permanent that is already tapped.
    #[error("{0:?} is already tapped")]
    AlreadyTapped(ObjectId),
    /// More floating mana was spent than is in the pool.
    #[error("not enough {0:?} mana in the pool")]
    PoolShortfall(ManaType),
    /// Restricted mana was spent on something its restriction does not allow.
    #[error("{0:?} mana may not be spent on this")]
    Restricted(ManaType),
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
    /// The mana chosen so far cannot all go toward the cost, so no completion exists.
    #[error("the chosen mana would be wasted")]
    Wasted,
    /// What is left cannot pay the rest of the cost.
    #[error("the rest of the cost cannot be paid from the remaining mana")]
    CannotComplete,
    /// The life paid is not an amount the cost takes: 2 for each Phyrexian symbol, no more.
    #[error(
        "paying {paid} life does not fit the cost (2 for each Phyrexian symbol, {most} at most)"
    )]
    BadLife {
        /// Life the payment pays.
        paid: u32,
        /// The most the cost could take.
        most: u32,
    },
    /// The payer has less life than the payment takes (CR 119.4).
    #[error("cannot pay {paid} life with a life total of {have}")]
    NotEnoughLife {
        /// Life the payment pays.
        paid: u32,
        /// The payer's life total.
        have: i32,
    },
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
    /// A rule requires an item that the answer leaves out.
    #[error("a rule requires the {0}")]
    Required(&'static str),
    /// The wrong number of items was chosen.
    #[error("expected {expected} items but got {got}")]
    WrongCount {
        /// Required number.
        expected: usize,
        /// Supplied number.
        got: usize,
    },
    /// An attacker that needs several blockers (menace) was blocked by fewer.
    #[error("the attacker needs {needed} blockers together but got {got}")]
    TooFewBlockers {
        /// The fewest creatures that may block it.
        needed: usize,
        /// Creatures the answer blocks it with.
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
    /// Damage was left for the player or planeswalker attacked before every blocker had lethal damage.
    #[error("damage may go past the blockers only after each has lethal damage")]
    ExcessBeforeLethal,
    /// The mana payment is invalid.
    #[error("invalid payment: {0}")]
    Payment(#[from] PaymentError),
    /// An amount was divided so that the shares do not add up to it.
    #[error("{expected} must be divided but the shares add up to {got}")]
    WrongTotal {
        /// The amount to divide.
        expected: u32,
        /// What the shares add up to.
        got: u32,
    },
}
