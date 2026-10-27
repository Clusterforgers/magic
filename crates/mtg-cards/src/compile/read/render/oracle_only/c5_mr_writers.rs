//! Wordings of the fifth sample of the names M to R that only the checker accepts, each derived from the wordings of a
//! plain twin of the ability (see [`c5_mr_twin`], which the checker writes in full) or from the wordings found before.

mod c5_mr_events;
mod c5_mr_grants;
mod c5_mr_moves;
mod c5_mr_sizes;
mod c5_mr_swaps;

use crate::ability::Ability;

/// Adds to `texts` the oracle wordings of the ability that the renderers do not write.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let json = serde_json::to_string(ability).unwrap_or_default();
    c5_mr_events::add(&json, texts);
    c5_mr_grants::add(ability, &json, texts);
    c5_mr_moves::add(ability, &json, texts);
    c5_mr_sizes::add(ability, &json, texts);
    // The swaps edit the wordings found by the modules above.
    c5_mr_swaps::add(&json, texts);
}

/// The wordings of the twin of the ability with the first `from` in its JSON changed to `to`; none when `from` is not
/// in it or the result is no ability.
fn c5_mr_twin(json: &str, from: &str, to: &str) -> Vec<String> {
    if !json.contains(from) {
        return Vec::new();
    }
    serde_json::from_str::<Ability>(&json.replacen(from, to, 1))
        .map(|plain| super::super::written_variants(&plain))
        .unwrap_or_default()
}

/// The wordings of the twin with each `from` of the pairs changed to its `to`, in order; none when one is missing.
fn c5_mr_twin_all(json: &str, pairs: &[(&str, &str)]) -> Vec<String> {
    let mut twin = json.to_owned();
    for (from, to) in pairs {
        if !twin.contains(from) {
            return Vec::new();
        }
        twin = twin.replacen(from, to, 1);
    }
    serde_json::from_str::<Ability>(&twin)
        .map(|plain| super::super::written_variants(&plain))
        .unwrap_or_default()
}

/// The texts that hold `from`, with it written as `to`.
fn c5_mr_swapped(texts: &[String], from: &str, to: &str) -> Vec<String> {
    texts
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replacen(from, to, 1))
        .collect()
}

/// The number right after `head` in the JSON when `close` follows it, and the text from `head` to the end of `close`.
fn c5_mr_number_after<'a>(json: &'a str, head: &str, close: &str) -> Option<(u32, &'a str)> {
    let at = json.find(head)?;
    let rest = json.get(at + head.len()..)?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let number = digits.parse().ok()?;
    let end = at + head.len() + digits.len() + close.len();
    let whole = json.get(at..end)?;
    rest.get(digits.len()..)?
        .starts_with(close)
        .then_some((number, whole))
}
