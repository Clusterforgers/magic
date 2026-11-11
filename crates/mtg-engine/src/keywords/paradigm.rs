//! Paradigm: "Then exile this spell. After you first resolve a spell with this name, you may cast a copy of
//! it from exile without paying its mana cost at the beginning of each of your first main phases."
//!
//! The replacement step exiles the card of a spell with paradigm as it resolves and, the first time its controller
//! resolves a spell with that name, sets up the delayed trigger that copies the exiled card and casts the copy at the
//! beginning of each of their precombat main phases for the rest of the game (`replace.rs`).

use super::KeywordRules;

/// The rules of paradigm.
#[derive(Debug)]
pub(super) struct Paradigm;

impl KeywordRules for Paradigm {}
