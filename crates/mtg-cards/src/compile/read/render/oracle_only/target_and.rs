//! Two targets of one ability joined in one sentence: "destroy up to one target creature and target player mills three
//! cards." The renderers write a sentence for each target.

use crate::ability::Ability;

/// Adds the wordings that join the sentence of a second target to the first by "and".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if ability.targets().len() != 2 {
        return;
    }
    let found: Vec<String> = texts.iter().filter_map(|text| joined(text)).collect();
    texts.extend(found);
}

/// The text with the sentence that opens with the second target joined to the one before it.
fn joined(text: &str) -> Option<String> {
    let (head, tail) = text
        .split_once(". Then target ")
        .or_else(|| text.split_once(". Target "))?;
    let tail = tail.strip_suffix('.').unwrap_or(tail);
    Some(format!("{head} and target {tail}."))
}
