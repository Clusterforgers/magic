//! A replacement that triples an amount: "it deals triple that damage instead", "three times that many of those
//! tokens are created instead". It words as the replacement that doubles does, with the multiplier changed.

use super::super::super::variants;
use crate::ability::Ability;
use crate::trigger::ReplaceBy;

/// The words of a doubling and the words of a tripling.
const SWAPS: [(&str, &str); 4] = [
    ("deals double that damage", "deals triple that damage"),
    ("twice that many", "three times that many"),
    ("twice that much", "three times that much"),
    ("draw two cards instead", "draw three cards instead"),
];

/// The wording of a doubling, tripled; none for a wording that does not double.
fn tripled(text: &str) -> Option<String> {
    SWAPS
        .iter()
        .find(|(doubled, _)| text.contains(doubled))
        .map(|(doubled, tripled)| text.replacen(doubled, tripled, 1))
}

/// Adds the wordings of a replacement that multiplies by three, derived from those of the one that multiplies by two.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        when,
        by: ReplaceBy::Multiply(3),
        condition,
        from,
    } = ability
    else {
        return;
    };
    let twin = Ability::Replacement {
        when: when.clone(),
        by: ReplaceBy::Multiply(2),
        condition: condition.clone(),
        from: *from,
    };
    texts.extend(variants(&twin).iter().filter_map(|text| tripled(text)));
}
