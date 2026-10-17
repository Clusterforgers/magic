//! Deterministic rules engine (no IO).
//!
//! A game is a pure fold of [`Event`]s over a [`GameSetup`]. [`Engine`] runs the game until a
//! player must decide, publishes that [`Decision`] with every legal option, and validates the
//! [`Answer`]; illegal answers never change state. Characteristics are always derived through
//! [`characteristics`], never stored.

mod answers;
mod cast;
mod damage;
mod flow;
mod mulligan;
mod sba;
mod steps;

pub mod characteristics;
pub mod combat;
pub mod decision;
pub mod effects;
pub mod engine;
pub mod error;
pub mod events;
pub mod fold;
pub mod format;
pub mod hash;
pub mod mana;
pub mod object;
pub mod player;
pub mod priority;
pub mod rng;
pub mod setup;
pub mod stack;
pub mod state;
pub mod turn;
pub mod zones;

pub use characteristics::{Characteristics, characteristics};
pub use decision::{Answer, Decision, DecisionKind};
pub use engine::{Checkpoint, Engine, replay};
pub use error::EngineError;
pub use events::Event;
pub use fold::apply_event;
pub use setup::{GameSetup, PlayerSetup, PresetPermanent, Start};
pub use state::{GameResult, GameState};
