//! A card compared with the mana value of the spell that triggered the ability: "..., return up to one target nonland
//! permanent an opponent controls with mana value less than or equal to that spell's mana value to its owner's hand."
//! The renderers name X and say "where X is that spell's mana value" at the end.

use crate::ability::Ability;
use crate::trigger::TriggerPattern;

/// The closing words the renderers add for the mana value of the spell.
const WHERE: &str = ", where X is that spell's mana value";

/// Adds the wordings that compare the mana value with the spell's directly.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::SpellCast { .. },
        ..
    } = ability
    else {
        return;
    };
    let said: Vec<String> = texts.iter().filter_map(|text| direct(text)).collect();
    texts.extend(said);
}

/// The text with "mana value X or less" as "less than or equal to that spell's mana value" and "mana value X" as "equal
/// to" it, and the closing words gone.
fn direct(text: &str) -> Option<String> {
    let without = text.replacen(WHERE, "", 1);
    if without == text {
        return None;
    }
    if without.contains("mana value X or less") {
        return Some(without.replacen(
            "mana value X or less",
            "mana value less than or equal to that spell's mana value",
            1,
        ));
    }
    without.contains("mana value X ").then(|| {
        without.replacen(
            "mana value X ",
            "mana value equal to that spell's mana value ",
            1,
        )
    })
}
