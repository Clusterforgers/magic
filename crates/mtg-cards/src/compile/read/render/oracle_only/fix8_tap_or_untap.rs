//! "You may tap or untap target creature.": choosing the state the permanent is already in does nothing, so the "may" of
//! a tap or untap adds no choice and the rule is the plain `TapOrUntap`.

/// Adds, for each wording with "tap or untap", the wording with "you may" before it.
pub(super) fn add(texts: &mut Vec<String>) {
    let found: Vec<String> = texts
        .iter()
        .filter(|text| !text.contains("may tap or untap"))
        .map(|text| {
            text.replace(": Tap or untap ", ": You may tap or untap ")
                .replace(", tap or untap ", ", you may tap or untap ")
                .replace(", then tap or untap ", ", then you may tap or untap ")
        })
        .chain(
            texts
                .iter()
                .filter_map(|text| text.strip_prefix("Tap or untap "))
                .map(|rest| format!("You may tap or untap {rest}")),
        )
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}
