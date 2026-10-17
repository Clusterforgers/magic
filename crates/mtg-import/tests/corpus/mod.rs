//! Goal: one place that knows how the export corpus is laid out and how to compare a parse with the truth, so each test file only says what it checks.

// Each test crate that includes this module uses only part of it.
#![allow(dead_code, unused_imports)]

mod aliases;
mod compare;
mod diff;
mod truth;

pub use aliases::*;
pub use compare::*;
pub use diff::*;
pub use truth::*;
