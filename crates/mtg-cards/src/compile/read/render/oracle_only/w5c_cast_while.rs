//! A permission to cast under a condition: "You may cast this card from your graveyard as long as you control three or
//! more tapped Pirates and/or Vehicles." The sentence is the permission without the condition, then "as long as" and
//! the condition; a count of two kinds also joins them with "and/or".

use super::super::written_variants;
use super::w4a_if_conditions::w4a_condition_texts;
use crate::ability::Ability;
use crate::modify::GameRule;
use crate::value::Condition;

/// Adds the wordings of a permission to cast that holds as long as a condition does.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule: rule @ GameRule::MayCast { .. },
        condition: Some(condition),
        from,
    } = ability
    else {
        return;
    };
    let bare = Ability::Rule {
        rule: rule.clone(),
        condition: None,
        from: *from,
    };
    let mut conditions = w4a_condition_texts(condition);
    if matches!(condition, Condition::Compare { .. }) {
        let joined: Vec<String> = conditions
            .iter()
            .filter_map(|text| {
                ["s or ", "s and "]
                    .iter()
                    .find(|joint| text.contains(*joint))
                    .map(|joint| text.replacen(joint, "s and/or ", 1))
            })
            .collect();
        conditions.extend(joined);
    }
    for text in written_variants(&bare) {
        let Some(permission) = text.strip_suffix('.') else {
            continue;
        };
        for condition in &conditions {
            texts.push(format!("{permission} as long as {condition}."));
        }
    }
}
