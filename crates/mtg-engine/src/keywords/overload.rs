//! Overload (CR 702.96): "You may cast this spell for its overload cost. If you do, change its text by replacing all
//! instances of 'target' with 'each'."
//!
//! The cost is an alternative cost chosen with the cast (`ways.rs`); an overloaded spell announces no targets and reads
//! each of its target references as every object the specification could target (`Scope::refs`).

use super::KeywordRules;

/// The rules of overload.
#[derive(Debug)]
pub(super) struct Overload;

impl KeywordRules for Overload {}
