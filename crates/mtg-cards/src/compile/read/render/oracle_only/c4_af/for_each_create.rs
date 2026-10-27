//! A count of tokens said with "for each": "For each land you control, create a Treasure token." and "..., for each
//! hatchling counter on it, create a 1/1 black Insect creature token with flying." The renderers write "create a number
//! of ... tokens equal to the number of ...".

use super::text::{json, singular};
use crate::ability::Ability;

/// Adds the "for each" wordings of an ability that creates a counted number of tokens.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let rules = json(ability);
    let counted = rules.contains(r#""count":{"Count""#) || rules.contains(r#""count":{"Counters""#);
    if !rules.contains(r#""CreateToken""#) || !counted {
        return;
    }
    let said: Vec<String> = texts.iter().flat_map(|text| for_each(text)).collect();
    texts.extend(said);
}

/// The wording with the count first, with either article for the token.
fn for_each(text: &str) -> Vec<String> {
    const MARKER: &str = "reate a number of ";
    let Some(at) = text.find(MARKER) else {
        return Vec::new();
    };
    let before = &text[..at];
    let (lead, opening) = if let Some(lead) = before.strip_suffix("you c") {
        (lead, "for each ")
    } else if let Some(lead) = before.strip_suffix('C') {
        (lead, "For each ")
    } else {
        return Vec::new();
    };
    let Some((what, after)) = text[at + MARKER.len()..].split_once(" equal to the number of ")
    else {
        return Vec::new();
    };
    let (per, tail) = after.split_at(after.find('.').unwrap_or(after.len()));
    let token = what.replacen(" tokens", " token", 1);
    ["a", "an"]
        .iter()
        .map(|article| {
            format!(
                "{lead}{opening}{}, create {article} {token}{tail}",
                singular(per)
            )
        })
        .collect()
}
