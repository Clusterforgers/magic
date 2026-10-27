//! The plural and the singular of the noun a counted phrase is about ("lands you control", "cards in your hand").

/// The words after which the head noun of a phrase has ended.
const MARKERS: [&str; 8] = [
    " you control",
    " your opponents control",
    " an opponent controls",
    " on it",
    " in your hand",
    " in your graveyard",
    " on the battlefield",
    " you own",
];

/// The phrase with the word before its first marker (else its last word) changed by `change`.
fn with_head(phrase: &str, change: fn(&str) -> String) -> String {
    let at = MARKERS
        .iter()
        .filter_map(|marker| phrase.find(marker))
        .min()
        .unwrap_or(phrase.len());
    let (head, tail) = phrase.split_at(at);
    match head.rsplit_once(' ') {
        Some((front, word)) => format!("{front} {}{tail}", change(word)),
        None => format!("{}{tail}", change(head)),
    }
}

/// The phrase with its head noun in the singular: "land you control".
pub(super) fn singular(phrase: &str) -> String {
    with_head(phrase, |word| match word.strip_suffix("ies") {
        Some(stem) => format!("{stem}y"),
        None => word.strip_suffix('s').unwrap_or(word).to_owned(),
    })
}

/// The phrase with its head noun in the plural: "cards in your hand".
pub(super) fn plural(phrase: &str) -> String {
    with_head(phrase, |word| match word.strip_suffix('y') {
        Some(stem) => format!("{stem}ies"),
        None => format!("{word}s"),
    })
}

/// The ability as JSON, which the guards search for the shape they want.
pub(super) fn json(ability: &crate::ability::Ability) -> String {
    serde_json::to_string(ability).unwrap_or_default()
}
