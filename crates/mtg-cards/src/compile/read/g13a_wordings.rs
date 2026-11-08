//! Round 13 wordings: a search of a library for "an Aura card and/or an Equipment card", which finds the same cards as
//! "an Aura card and an Equipment card".

/// Adds the "and/or" wording of each search for two cards of the texts.
pub(super) fn add(texts: &mut Vec<String>) {
    let found: Vec<String> = texts.iter().filter_map(|text| and_or(text)).collect();
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "Search your library for an Aura card and an Equipment card" as "an Aura card and/or an Equipment card": a search
/// of a hidden zone may find either card or neither (CR 701.23b), so the two say the same.
fn and_or(text: &str) -> Option<String> {
    let at = text.find("library for a")? + "library for ".len();
    let end = text[at..].find(", ").map_or(text.len(), |end| at + end);
    let join = text[at..end].find(" card and a")? + at + " card".len();
    Some(format!(
        "{} and/or{}",
        &text[..join],
        &text[join + " and".len()..]
    ))
}
