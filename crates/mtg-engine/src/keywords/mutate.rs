//! Mutate (CR 702.140): "You may cast this card for its mutate cost." The cost is an alternative cost chosen with the cast
//! from a hand (`ways.rs`); the merge, the `Mutates` trigger and the merged permanent are in `merged.rs`. The keyword adds
//! no ability.

use super::KeywordRules;

/// The rules of mutate.
#[derive(Debug)]
pub(super) struct Mutate;

impl KeywordRules for Mutate {}
