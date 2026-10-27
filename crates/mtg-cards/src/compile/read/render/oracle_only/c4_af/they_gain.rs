//! A group that gets counters and then a keyword: "Put a +1/+1 counter on each legendary creature you control. They gain
//! vigilance until end of turn." The renderers repeat the group after "Then".

use super::text::json;
use crate::ability::Ability;

/// The array that follows `marker` in `rules`, up to its closing bracket.
fn array_after(rules: &str, marker: &str) -> Option<String> {
    let rest = &rules[rules.find(marker)? + marker.len()..];
    let mut depth = 0_usize;
    for (at, c) in rest.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(rest[..=at].to_owned());
                }
            }
            _ => {}
        }
    }
    None
}

/// Adds the wordings that call the group "They" or "Those creatures" when counters and a keyword go to the same group.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let rules = json(ability);
    let counted = array_after(&rules, r#""PutCounters":{"on":{"Each":"#);
    let gaining = array_after(&rules, r#""Continuous":{"affected":{"Each":"#);
    if counted.is_none() || counted != gaining {
        return;
    }
    let said: Vec<String> = texts.iter().flat_map(|text| pronoun(text)).collect();
    texts.extend(said);
}

/// The text with the group of its "Then ... gain" sentence named by a pronoun.
fn pronoun(text: &str) -> Vec<String> {
    let Some(at) = text.find(". Then ") else {
        return Vec::new();
    };
    let rest = &text[at + ". Then ".len()..];
    let Some((subject, verb)) = ["gains ", "gain "]
        .iter()
        .filter_map(|verb| rest.find(verb).map(|found| (found, verb)))
        .min()
        .map(|(found, verb)| (&rest[..found], verb))
    else {
        return Vec::new();
    };
    if subject.contains('.') {
        return Vec::new();
    }
    let after = &rest[subject.len() + verb.len()..];
    ["They", "Those creatures"]
        .iter()
        .map(|pronoun| format!("{}. {pronoun} gain {after}", &text[..at]))
        .collect()
}
