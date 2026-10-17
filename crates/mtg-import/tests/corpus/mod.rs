//! Shared helpers of the export-corpus tests: the truth model and loader, name aliases, and the comparison of a parse with the truth.

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
