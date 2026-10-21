//! Convoke (CR 702.51): the payment of the cost reads the keyword (`help.rs`), so it answers no hook.

use super::KeywordRules;

/// The rules of convoke.
#[derive(Debug)]
pub(super) struct Convoke;

impl KeywordRules for Convoke {}
