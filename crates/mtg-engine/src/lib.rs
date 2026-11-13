//! Deterministic rules engine (no IO).
//!
//! A game is a pure fold of [`Event`]s over a [`GameSetup`]. [`Engine`] runs the game until a
//! player must decide, publishes that [`Decision`] with every legal option, and validates the
//! [`Answer`]; illegal answers never change state. Characteristics are always derived through
//! [`characteristics`], never stored.

mod activation_kind;
mod announce;
mod answers;
mod attach;
mod attack_tax;
pub mod battle;
pub mod block_tax;
mod c10_xp_copy_ends;
mod c11a_less;
mod c7_untap;
mod cant_named;
mod cast;
mod combat_quirks;
mod commanders;
mod costly_mana;
mod damage;
#[cfg(test)]
mod e14a_spend_facts;
mod encoded;
mod exile_return;
mod face_down;
mod faces;
#[cfg(test)]
mod fixtures;
mod flow;
mod free_activation;
#[cfg(test)]
mod help_tests;
mod l292_haunt;
mod land_types;
mod layers;
mod legal_declaration;
#[cfg(test)]
mod legal_declaration_tests;
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
mod mana_replace;
mod mana_snow;
#[cfg(test)]
mod mana_snow_tests;
#[cfg(test)]
mod mana_sources_tests;
mod mana_takes;
mod mulligan;
mod parts;
mod permit_costs;
#[cfg(test)]
mod pf23_hand_mana_tests;
mod prepared;
mod preview;
mod price;
mod r303b_blocked_by;
mod r303b_haste_at;
mod r372_suspend;
mod r377_skips;
mod r379_since;
mod r401_riders;
mod replace_dry;
mod rmt3_skips;
mod saga;
mod sba;
mod sneak;
mod soulbond;
mod spell_keywords;
mod spend_rider;
#[cfg(test)]
mod spend_tests;
mod steps;
mod stun;
mod teams;
mod unplayable;
mod w10a_attack_limits_at;
mod w11a_exert;
mod w14c_block_capacity;
mod w7b_rad;
mod w8a_declaration_limits;
mod ways;
mod why;
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
pub mod conjure;
pub mod copies;
pub mod counters;
pub mod day_night;
pub mod decision;
pub mod dk1c5_rooms;
pub(crate) mod e15g_paid;
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
mod l294_damage;
pub mod library;
pub mod loyalty;
pub mod mana;
pub mod manual;
mod manual_control;
mod manual_effects;
mod manual_faces;
pub mod manual_grant;
mod manual_library;
mod manual_roll;
mod manual_show;
pub mod manual_tokens;
pub mod memory;
pub mod merged;
pub mod monarch;
pub mod object;
mod party;
pub mod phasing;
pub mod player;
pub mod player_control;
pub mod priority;
pub mod prohibit;
mod r281_kicked_spend;
mod r281_mana_at_priority;
pub mod r286_joins;
mod r300c_activation_spent;
mod r301_offering;
#[cfg(test)]
mod r301b_spend_facts;
mod r304b_lost_priority;
mod r371_token_loyalty;
mod r400_attack_follow;
mod r400_no_mana;
mod redeal;
pub mod replace;
pub mod resolve;
mod restart;
pub mod retargeted;
mod ring;
mod rmt4_untap_limit;
#[cfg(test)]
mod rmt6_pieces_tests;
mod rmt6_storied;
mod rmt8_turns;
pub mod rng;
#[cfg(test)]
mod rule_options;
pub mod rules;
pub mod setup;
pub mod sight;
pub mod smart_stops;
pub mod snapshot;
pub mod speed;
pub mod spend;
pub mod stack;
pub mod state;
pub mod take_back;
pub mod text_change;
pub mod tokens;
pub mod triggers;
pub mod turn;
mod turn_log;
#[cfg(test)]
mod turn_log_tests;
mod turn_summary;
pub mod un33_attractions;
pub mod un34_stickers;
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
pub use mana_ability::mana_ways;
pub use setup::{GameSetup, PlayerSetup, PresetPermanent, Start};
pub use snapshot::{Snapshot, snapshot};
pub use state::{GameResult, GameState};
#[cfg(test)]
mod r376_nearest_tests;
