//! A rule that holds under a condition: "As long as an opponent controls more lands than you, you may play lands from
//! the top of your library." The renderers have no wording for the condition of a rule.

use super::super::{condition_text, variants};
use super::bare::lowered;
use crate::ability::Ability;

/// Adds the wordings of a rule with a condition, worded as the rule without it after "As long as".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule,
        condition: Some(condition),
        from,
    } = ability
    else {
        return;
    };
    let Some(held) = condition_text(condition) else {
        return;
    };
    let plain = Ability::Rule {
        rule: rule.clone(),
        condition: None,
        from: *from,
    };
    texts.extend(
        variants(&plain)
            .iter()
            .map(|text| format!("As long as {held}, {}", lowered(text))),
    );
}
