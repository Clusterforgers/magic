//! A cost in energy that a card writes as a number: "Pay six {E}". The renderers write one {E} for each counter.

use super::super::super::amounts::count_word;
use crate::ability::Ability;
use crate::cost::CostItem;
use crate::value::Amount;

/// Adds the wordings of an activation that pays more than one energy counter with the number in words.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated { cost, .. } = ability else {
        return;
    };
    let mut found = Vec::new();
    for item in &cost.0 {
        let CostItem::PayEnergy(Amount::Fixed(energy)) = item else {
            continue;
        };
        let Ok(energy) = u32::try_from(*energy) else {
            continue;
        };
        let Some(word) = count_word(energy).filter(|_| energy > 1) else {
            continue;
        };
        let (symbols, words) = (
            format!("Pay {}", "{E}".repeat(energy as usize)),
            format!("Pay {word} {{E}}"),
        );
        found.extend(
            texts
                .iter()
                .filter(|text| text.contains(&symbols))
                .map(|text| text.replacen(&symbols, &words, 1)),
        );
    }
    texts.extend(found);
}
