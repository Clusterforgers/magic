//! The four party roles named as a list: the renderers join a list of subtypes with "or" ("Cleric, Rogue, Warrior, or
//! Wizard spells"), the oracle text of a party card with "and" ("Cleric, Rogue, Warrior, and Wizard spells") or, with
//! the Ally of Tazri, "and/or".

/// The lists as the renderers write them and as the party cards word them.
const LISTS: [(&str, &str); 2] = [
    (
        "Cleric, Rogue, Warrior, or Wizard",
        "Cleric, Rogue, Warrior, and Wizard",
    ),
    (
        "Cleric, Rogue, Warrior, Wizard, or Ally",
        "Cleric, Rogue, Warrior, Wizard, and/or Ally",
    ),
];

/// Adds the wordings that join the party roles with "and" or "and/or".
pub(super) fn add(texts: &mut Vec<String>) {
    let said: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            LISTS
                .iter()
                .filter(|(or, _)| text.contains(or))
                .map(|(or, said)| text.replace(or, said))
        })
        .collect();
    texts.extend(said);
}
