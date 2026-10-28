//! A second instruction of the controller joined to the first: where a wording reads "A. Then B" with B an
//! instruction to the controller, the card prints "A and you B" ("each opponent loses X life and you scry X").

/// The verbs of an instruction to the controller, as they open the instruction after the join.
const C6_SZ_VERBS: [&str; 8] = [
    "draw ", "scry ", "return ", "gain ", "create ", "mill ", "surveil ", "put ",
];

/// Adds, for each wording with ". Then " before an instruction, the wording joined with "and you".
pub(super) fn add(texts: &mut Vec<String>) {
    let joined: Vec<String> = texts
        .iter()
        .filter_map(|text| c6_sz_and_you(text))
        .collect();
    texts.extend(joined);
}

/// The wording with its first ". Then " and the instruction after it joined as "and you <instruction>".
fn c6_sz_and_you(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(". Then ")?;
    let mut chars = tail.chars();
    let instruction = format!("{}{}", chars.next()?.to_ascii_lowercase(), chars.as_str());
    C6_SZ_VERBS
        .iter()
        .any(|verb| instruction.starts_with(verb))
        .then(|| format!("{head} and you {instruction}"))
}
