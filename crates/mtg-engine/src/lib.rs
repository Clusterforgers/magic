//! Deterministic rules engine (no IO).
//!
//! A game is a pure fold of [`Event`]s over a [`GameSetup`]. [`Engine`] runs the game until a
//! player must decide, publishes that [`Decision`] with every legal option, and validates the
//! [`Answer`]; illegal answers never change state. Characteristics are always derived through
//! [`characteristics`], never stored.

mod answers;
mod cast;
mod damage;
#[cfg(test)]
mod fixtures;
mod flow;
mod layers;
mod mulligan;
mod preview;
mod price;
mod sba;
mod steps;
mod wording;

pub mod abilities;
pub mod ability_reader;
pub mod caps;
pub mod characteristics;
pub mod choose;
pub mod combat;
pub mod counters;
pub mod decision;
pub mod effects;
pub mod engine;
pub mod error;
pub mod events;
pub mod fold;
pub mod forecast;
pub mod format;
pub mod hash;
pub mod keywords;
pub mod library;
pub mod mana;
pub mod object;
pub mod player;
pub mod priority;
pub mod replace;
pub mod resolve;
pub mod rng;
pub mod setup;
pub mod sight;
pub mod snapshot;
pub mod stack;
pub mod state;
pub mod tokens;
pub mod triggers;
pub mod turn;
pub mod zones;

pub use caps::caps;
pub use characteristics::{Characteristics, characteristics};
pub use decision::{Answer, Decision, DecisionKind};
pub use engine::{Checkpoint, Engine, replay};
pub use error::{EngineError, PreviewError};
pub use events::Event;
pub use fold::apply_event;
pub use setup::{GameSetup, PlayerSetup, PresetPermanent, Start};
pub use snapshot::{Snapshot, snapshot};
pub use state::{GameResult, GameState};
