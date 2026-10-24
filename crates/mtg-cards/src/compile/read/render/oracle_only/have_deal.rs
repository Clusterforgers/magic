//! Damage the source may deal: "you may have this enchantment deal 5 damage to any target". The renderers write the
//! choice with the source as the subject: "you may ~ deals 5 damage".

/// Adds the wordings of an optional damage by the source with "have ... deal".
pub(super) fn add(texts: &mut Vec<String>) {
    let (from, into) = (", you may ~ deals ", ", you may have ~ deal ");
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replacen(from, into, 1))
        .collect();
    texts.extend(said);
}
