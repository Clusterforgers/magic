//! Delve (CR 702.66): the payment of the cost reads the keyword (`help.rs`), so it answers no hook.

use super::KeywordRules;

/// The rules of delve.
#[derive(Debug)]
pub(super) struct Delve;

impl KeywordRules for Delve {}
