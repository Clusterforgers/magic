//! Wordings of the r60-07 pack beside the writers' own: a Jace card empowers itself by its name, so its text reads
//! "Empower ~ X" where another card prints "empower Jace X".

/// Adds "empower ~" beside each "empower Jace" of the texts.
pub(super) fn r6007_wordings(texts: &mut Vec<String>) {
    let more: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("mpower Jace "))
        .map(|text| text.replace("mpower Jace ", "mpower ~ "))
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(more);
}
