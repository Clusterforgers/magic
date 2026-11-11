//! Storied: the enduring story is given by the state-based actions (`rmt6_storied.rs`), so the keyword answers no hook.

use super::KeywordRules;

/// The rules of storied.
#[derive(Debug)]
pub(super) struct Storied;

impl KeywordRules for Storied {}
