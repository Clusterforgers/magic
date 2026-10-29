//! A later step joined by ", then": "Draw three cards, then discard four cards." The renderers write it as a sentence
//! of its own that opens with "Then".

/// Adds, for each wording with one sentence that opens with "Then", the wording with that sentence joined by ", then".
pub(super) fn add(texts: &mut Vec<String>) {
    let joined: Vec<String> = texts
        .iter()
        .filter(|text| text.matches(". Then ").count() == 1)
        .map(|text| text.replace(". Then ", ", then "))
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(joined);
}
