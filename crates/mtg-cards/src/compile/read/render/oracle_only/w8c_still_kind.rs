//! Permanents of a type that become creatures of a size and keep their type, said as the oracle does: "All lands
//! become 2/2 creatures until end of turn. They're still lands." The renderers write "become land creatures with base
//! power and toughness 2/2".

use super::super::variants;
use crate::ability::Ability;

/// The card types a group may keep as it becomes creatures.
const W8C_KINDS: [&str; 3] = ["land", "artifact", "enchantment"];

/// Adds the wordings of a group of one type that becomes creatures with a base size.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let said: Vec<String> = variants(ability)
        .iter()
        .filter_map(|text| W8C_KINDS.iter().find_map(|kind| w8c_still(text, kind)))
        .collect();
    texts.extend(said);
}

/// The wording "all <kind>s become P/T creatures ... They're still <kind>s." of one sentence that says the size.
fn w8c_still(text: &str, kind: &str) -> Option<String> {
    let plain = format!("All {kind}s become {kind} creatures with base power and toughness ");
    let rest = text.strip_prefix(&plain)?;
    let (size, after) = rest.split_once(' ')?;
    let body = after.strip_suffix('.')?;
    if body.contains('.') || !size.contains('/') {
        return None;
    }
    Some(format!(
        "All {kind}s become {size} creatures {body}. They're still {kind}s."
    ))
}
