//! A check of the counters after they were put: "Whenever you gain life, put that many ribbon counters on this creature.
//! Then if there are three or more ribbon counters on this creature, remove those counters and untap it." The renderers
//! write the removal first and the check last.

use crate::ability::Ability;

/// Adds the wordings that put the check of the counters before the removal and the untap.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !matches!(ability, Ability::Triggered { .. }) {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(". Then remove all of them and untap it if there are ")?;
            let (counters, _) = rest.strip_suffix(" on it.")?.rsplit_once(" counters")?;
            Some(format!(
                "{head}. Then if there are {counters} counters on ~, remove those counters and untap it."
            ))
        })
        .collect();
    texts.extend(said);
}
