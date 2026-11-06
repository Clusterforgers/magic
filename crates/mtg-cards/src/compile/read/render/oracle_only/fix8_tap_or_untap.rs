//! "You may tap or untap target creature.": choosing the state the permanent is already in does nothing, so the "may" of
//! a tap or untap adds no choice and the rule is the plain `TapOrUntap`.

/// Adds, for each wording with "tap or untap", the wording with "you may" before it.
pub(super) fn add(texts: &mut Vec<String>) {
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("ap or untap ") && !text.contains("may tap or untap"))
        .map(|text| {
            let said = text
                .replace(": Tap or untap ", ": You may tap or untap ")
                .replace(", tap or untap ", ", you may tap or untap ")
                .replace(", then tap or untap ", ", then you may tap or untap ");
            match said.strip_prefix("Tap or untap ") {
                Some(rest) => format!("You may tap or untap {rest}"),
                None => said,
            }
        })
        .filter(|text| text.contains("may tap or untap"))
        .collect();
    texts.extend(found);
}
