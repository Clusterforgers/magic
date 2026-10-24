//! Two creature types of a group, which the oracle may join with "and/or": "put a +1/+1 counter on each other Merfolk
//! and/or Knight you control". The renderers write "or".

/// Adds the wordings of a group of two subtypes joined with "and/or".
pub(super) fn add(texts: &mut Vec<String>) {
    let said: Vec<String> = texts.iter().filter_map(|text| and_or(text)).collect();
    texts.extend(said);
}

/// The text with the "or" between the two subtypes after "each " or "each other " changed; none without such a pair.
fn and_or(text: &str) -> Option<String> {
    let start = text
        .find("each other ")
        .map(|at| at + "each other ".len())
        .or_else(|| text.find("each ").map(|at| at + "each ".len()))?;
    let (head, tail) = text.split_at(start);
    let (first, rest) = tail.split_once(" or ")?;
    let second = rest.split([' ', ',', '.']).next().unwrap_or("");
    (is_subtype(first) && is_subtype(second)).then(|| format!("{head}{first} and/or {rest}"))
}

/// A creature type is one capitalized word; a card type or other noun is lowercase.
fn is_subtype(word: &str) -> bool {
    !word.contains(' ') && word.chars().next().is_some_and(char::is_uppercase)
}
