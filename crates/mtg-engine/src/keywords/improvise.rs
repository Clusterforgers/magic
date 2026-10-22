//! Improvise (CR 702.126): the payment of the cost reads the keyword (`help.rs`), so it answers no hook.

use super::KeywordRules;

/// The rules of improvise.
#[derive(Debug)]
pub(super) struct Improvise;

impl KeywordRules for Improvise {}
