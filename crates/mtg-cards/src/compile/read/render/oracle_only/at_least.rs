//! A count of other creatures a condition asks for: "as long as you control at least two other Merfolk". The renderers
//! write "two or more other Merfolk".

use super::super::count_word;
use crate::ability::Ability;

/// Adds the wordings that say "at least" for "or more" in a condition on what you control.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !matches!(
        ability,
        Ability::Static {
            condition: Some(_),
            ..
        }
    ) {
        return;
    }
    let mut said = Vec::new();
    for number in 2..=9 {
        let Some(word) = count_word(number) else {
            continue;
        };
        let (from, into) = (
            format!("as long as you control {word} or more other "),
            format!("as long as you control at least {word} other "),
        );
        said.extend(
            texts
                .iter()
                .filter(|text| text.contains(&from))
                .map(|text| text.replacen(&from, &into, 1)),
        );
    }
    texts.extend(said);
}
