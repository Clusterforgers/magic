//! The duration inside a sentence: "Whenever you cast a colorless spell, until end of turn, this creature gains trample
//! and gets +X/+X." The renderers capitalize the duration they put first.

/// Adds the wordings with the duration after a comma in lower case.
pub(super) fn add(texts: &mut Vec<String>) {
    let (from, into) = (", Until end of turn, ", ", until end of turn, ");
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replace(from, into))
        .collect();
    texts.extend(said);
}
