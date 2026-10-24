//! A group that excludes a supertype: "Exile all nonland permanents that aren't legendary." The renderers write
//! "nonland, nonlegendary permanents".

/// Adds the wordings that say the supertype after the group.
pub(super) fn add(texts: &mut Vec<String>) {
    let (from, into) = (
        "nonland, nonlegendary permanents",
        "nonland permanents that aren't legendary",
    );
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replace(from, into))
        .collect();
    texts.extend(said);
}
