//! The "target ..." phrases of a text, read in order by the reader the compiler uses for printed text.

use super::selectors::spec;
use super::tokens::{Reader, canonical, tokenize};
use crate::refs::TargetSpec;

/// The target specifications the phrases of the text declare, in the order the text words them: "Counter target
/// noncreature spell." declares one. A phrase the reader does not understand declares nothing.
pub fn target_phrases(text: &str) -> Vec<TargetSpec> {
    let tokens = tokenize(&canonical(text));
    let mut reader = Reader::new(&tokens, false);
    let mut found = Vec::new();
    while !reader.end() {
        match reader.attempt(spec) {
            Some(read) => found.push(read),
            None => reader.skip(),
        }
    }
    found
}
