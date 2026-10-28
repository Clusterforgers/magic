//! Wordings of the sixth sample of the names M to R that only the checker accepts, each derived from the wordings of a
//! plain twin of the ability (see [`c6_mr_twin_all`], which the checker writes in full) or of its parts.

mod c6_mr_events;
mod c6_mr_looks;
mod c6_mr_modes;
mod c6_mr_moves;
mod c6_mr_swaps;

use crate::ability::Ability;

/// Adds to `texts` the oracle wordings of the ability that the renderers do not write.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let json = serde_json::to_string(ability).unwrap_or_default();
    c6_mr_events::add(&json, texts);
    c6_mr_looks::add(&json, texts);
    c6_mr_moves::add(&json, texts);
    c6_mr_modes::add(&json, texts);
    // The swaps edit the wordings found by the modules above.
    c6_mr_swaps::add(&json, texts);
}

/// The wordings the checker accepts for the ability written as `json`; none when it is no ability.
fn c6_mr_written(json: &str) -> Vec<String> {
    serde_json::from_str::<Ability>(json)
        .map(|plain| super::super::written_variants(&plain))
        .unwrap_or_default()
}

/// The wordings of the twin with each `from` of the pairs changed to its `to`, in order; none when one is missing.
fn c6_mr_twin_all(json: &str, pairs: &[(&str, &str)]) -> Vec<String> {
    let mut twin = json.to_owned();
    for (from, to) in pairs {
        if !twin.contains(from) {
            return Vec::new();
        }
        twin = twin.replacen(from, to, 1);
    }
    c6_mr_written(&twin)
}

/// The texts that hold `from`, with it written as `to`.
fn c6_mr_swapped(texts: &[String], from: &str, to: &str) -> Vec<String> {
    texts
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replacen(from, to, 1))
        .collect()
}

/// The name bound right after `head` in the JSON: the text up to the next quote.
fn c6_mr_name_after<'a>(json: &'a str, head: &str) -> Option<&'a str> {
    let at = json.find(head)? + head.len();
    let rest = json.get(at..)?;
    rest.get(..rest.find('"')?)
}
