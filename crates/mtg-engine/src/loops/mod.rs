//! Loops (CR 104.4b, CR 732): the engine notes the state at every priority decision, finds a cycle that repeats, and
//! either ends the game in a draw, runs it to its end, or offers its driver a shortcut of a number of iterations.

mod shape;
mod watch;

use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

pub(crate) use shape::shape as shape_at;
pub(crate) use watch::{Found, How, Taken, Watch, absolute};

/// The most iterations one shortcut runs, and the most a turn runs in all.
pub const LOOP_CAP: u32 = 10_000;

/// The work one shortcut may do: the permanents on the battlefield summed over its iterations. Every step of the engine
/// reads every permanent, so a loop that makes a permanent each time costs the square of its iterations; the shortcut
/// stops once it has done this much (a few hundred iterations of a growing loop, thousands of one that stays small).
pub const LOOP_WORK: usize = 60_000;

/// A loop offered to the player whose choice drives it: repeat it a number of times, or stop (CR 732.2a, 732.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LoopOptions {
    /// The names of the objects whose spells and abilities make up one iteration, in the order they first appear.
    pub cards: Vec<String>,
    /// What one iteration changes.
    pub per_iteration: LoopStep,
    /// No player made a choice in the loop: it runs by itself, and the asked player is the first who could respond.
    pub mandatory: bool,
    /// Something runs out (life), so the loop ends by itself.
    pub bounded: bool,
    /// The most iterations the answer may ask for (0 once the turn has run its cap).
    pub max: u32,
    /// How many times this loop was offered before in this turn.
    pub asked: u32,
    /// The loop never ends by itself and no one has to act: the asked player may let it go on, and the game is a draw
    /// (CR 104.4b).
    pub can_draw: bool,
}

/// What one iteration of a loop changes, public to every seat.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LoopStep {
    /// Tokens created: controller, name and count.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<LoopTokens>>", optional))]
    pub tokens: Vec<LoopTokens>,
    /// Life totals that changed: player and change.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<LoopLife>>", optional))]
    pub life: Vec<LoopLife>,
}

/// Tokens of one name a loop created for one controller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LoopTokens {
    /// Who controls them.
    pub controller: PlayerId,
    /// Their name.
    pub name: String,
    /// How many.
    pub count: u32,
}

/// The change of one player's life over a loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LoopLife {
    /// Whose life.
    pub player: PlayerId,
    /// The change.
    pub change: i64,
}

/// The answer to a loop offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum LoopAnswer {
    /// Run the loop this many more times in one batch, then play on (at most `LoopOptions::max`, at least 1).
    Repeat(u32),
    /// Play on without a shortcut: the player makes a different choice (or responds) by hand.
    Stop,
    /// Let a loop that never ends by itself go on: the game is a draw (`LoopOptions::can_draw`).
    Draw,
}

/// A shortcut the engine ran: the iterations and what they changed, in place of the events it folds (protocol).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LoopSummary {
    /// Who chose the number; none for a loop of mandatory actions run to its end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub player: Option<PlayerId>,
    /// The names of the objects whose spells and abilities make up one iteration.
    pub cards: Vec<String>,
    /// The iterations run.
    pub iterations: u32,
    /// What they changed in all.
    pub total: LoopStep,
    /// How many log events right before this one the shortcut made.
    pub events: u32,
}

/// The offer the engine waits on, with the iteration it repeats.
#[derive(Debug, Clone)]
pub(crate) struct Offer {
    /// The loop found.
    pub found: Found,
    /// The question shown.
    pub options: LoopOptions,
}
