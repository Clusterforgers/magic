//! A comma after a quoted ability: `has "Whenever this creature attacks, untap target attacking creature," and is a
//! Cleric in addition to its other types.` The renderers close the quote with a period.

use crate::ability::Ability;
use crate::modify::Modification;

/// Adds the wordings that put the comma inside the quote when more of the sentence follows.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static { mods, .. } = ability else {
        return;
    };
    if !mods
        .iter()
        .any(|modification| matches!(modification, Modification::GrantAbility(_)))
    {
        return;
    }
    let (from, into) = (".\", and ", ",\" and ");
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replacen(from, into, 1))
        .collect();
    texts.extend(said);
}
