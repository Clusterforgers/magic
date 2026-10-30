//! Deterministic rules engine (no IO).
//!
//! A game is a pure fold of [`Event`]s over a [`GameSetup`]. [`Engine`] runs the game until a
//! player must decide, publishes that [`Decision`] with every legal option, and validates the
//! [`Answer`]; illegal answers never change state. Characteristics are always derived through
//! [`characteristics`], never stored.

mod announce;
mod answers;
mod attach;
mod attack_tax;
mod c10_xp_copy_ends;
mod c11a_less;
mod c7_untap;
mod cast;
mod commanders;
mod damage;
mod encoded;
mod exile_return;
mod face_down;
mod faces;
#[cfg(test)]
mod fixtures;
mod flow;
#[cfg(test)]
mod help_tests;
mod layers;
mod legend;
mod mana_ability;
mod mana_bonus;
mod mana_cost;
mod mana_kinds;
#[cfg(test)]
mod mana_lasts_tests;
mod mana_plan;
mod mana_repeat;
#[cfg(test)]
mod mana_repeat_tests;
mod mana_takes;
mod mulligan;
mod parts;
mod prepared;
mod preview;
mod price;
mod replace_dry;
mod saga;
mod sba;
mod spell_keywords;
#[cfg(test)]
mod spend_tests;
mod steps;
mod w10a_attack_limits_at;
mod w11a_exert;
mod w14c_block_capacity;
mod w7b_rad;
mod w8a_declaration_limits;
mod ways;
mod wording;
#[cfg(test)]
mod zone_abilities;

pub mod abilities;
pub mod ability_reader;
mod c9_emerge_ways;
pub mod caps;
pub mod characteristics;
pub mod choose;
mod citys_blessing;
pub mod combat;
pub mod companions;
pub mod copies;
pub mod counters;
pub mod day_night;
pub mod decision;
pub mod effects;
pub mod engine;
pub mod error;
pub mod events;
pub mod fold;
pub mod forecast;
pub mod format;
pub mod gives;
pub mod hash;
pub mod help;
pub mod keywords;
pub mod library;
pub mod loyalty;
pub mod mana;
pub mod manual;
mod manual_effects;
pub mod merged;
pub mod monarch;
pub mod object;
mod party;
pub mod phasing;
pub mod player;
pub mod priority;
pub mod replace;
pub mod resolve;
pub mod rng;
pub mod rules;
pub mod setup;
pub mod sight;
pub mod snapshot;
pub mod spend;
pub mod stack;
pub mod state;
pub mod tokens;
pub mod triggers;
pub mod turn;
pub mod w11d_dungeon;
pub mod w15a_initiative;
pub mod w17b_statics;
pub mod w6b_emblems;
pub mod zones;

#[cfg(test)]
mod w9c_saturation;

pub use caps::caps;
pub use characteristics::{Characteristics, characteristics};
pub use day_night::DayNight;
pub use decision::{Answer, Decision, DecisionKind, FaceOption};
pub use engine::{Checkpoint, Engine, replay};
pub use error::{EngineError, PreviewError};
pub use events::Event;
pub use fold::apply_event;
pub use setup::{GameSetup, PlayerSetup, PresetPermanent, Start};
pub use snapshot::{Snapshot, snapshot};
pub use state::{GameResult, GameState};
