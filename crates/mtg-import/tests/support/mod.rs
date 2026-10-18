//! Goal: shared by the mana cost tests: what a Scryfall cost string may come to (a cost, or a refusal naming a symbol the engine leaves out).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use mtg_core::mana::{ManaCost, ParseManaCostError};

/// Symbols the engine does not model: X and its siblings (roadmap M3 "X costs"), snow, and the silver-border and playtest ones.
pub const OUT_OF_SCOPE: &[&str] = &["D", "HW", "L", "S", "X", "Y", "Z"];

/// What reading one cost string came to.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The costs, one per face.
    Parsed(Vec<ManaCost>),
    /// The first symbol the engine does not model.
    Refused(String),
}

/// Reads a cost string (`A // B` for several faces) and insists that a refusal names a symbol on [`OUT_OF_SCOPE`].
pub fn verdict(text: &str) -> Verdict {
    match ManaCost::parse_faces(text) {
        Ok(costs) => Verdict::Parsed(costs),
        Err(ParseManaCostError::Unsupported(symbol)) => {
            assert!(
                OUT_OF_SCOPE.contains(&symbol.as_str()),
                "{text}: {{{symbol}}} is refused but is not a symbol left out on purpose"
            );
            assert!(
                text.contains(&format!("{{{symbol}}}")),
                "{text}: the error names {{{symbol}}}, which is not in it"
            );
            Verdict::Refused(symbol)
        }
        Err(other) => panic!("{text}: {other}"),
    }
}

/// The text inside every `{...}` of `text`.
pub fn symbols_of(text: &str) -> Vec<&str> {
    text.split('{')
        .skip(1)
        .filter_map(|rest| rest.split_once('}'))
        .map(|(symbol, _)| symbol)
        .collect()
}
