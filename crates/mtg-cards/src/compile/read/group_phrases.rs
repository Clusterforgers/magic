//! The groups a text names ("all lands you control", "Creatures you control get ..."), read by the reader the
//! compiler uses for printed text.

use super::selectors::{Form, all, phrase};
use super::tokens::{Reader, canonical, tokenize};
use crate::refs::{Ref, Selector};

/// The groups the words "all ..." name, in the order the text words them: "untap all lands you control" has one.
pub fn all_groups(text: &str) -> Vec<Selector> {
    let tokens = tokenize(&canonical(text));
    let mut reader = Reader::new(&tokens, false);
    let mut found = Vec::new();
    while !reader.end() {
        match reader.attempt(all) {
            Some(Ref::Each(selector)) => found.push(selector),
            Some(_) => {}
            None => reader.skip(),
        }
    }
    found
}

/// The group a sentence starts with, as the subject of a static ability: "Creatures you control have lifelink."
pub fn subject_group(text: &str) -> Option<Selector> {
    let tokens = tokenize(&canonical(text));
    let mut reader = Reader::new(&tokens, false);
    reader.attempt(|r| phrase(r, Form::All))
}
