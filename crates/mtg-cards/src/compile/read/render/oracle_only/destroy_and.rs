//! Two groups destroyed in one sentence: "Destroy all creatures you don't control and all planeswalkers you don't
//! control." The renderers write a sentence for each group.

use super::super::super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wordings that join the destruction of a second group to the first with "and all".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut said = Vec::new();
    for pair in ability.effects().windows(2) {
        let [
            Effect::Destroy {
                what: Ref::Each(_), ..
            },
            Effect::Destroy {
                what: Ref::Each(second),
                ..
            },
        ] = pair
        else {
            continue;
        };
        let Some(all) = phrase_text(second, Form::All) else {
            continue;
        };
        let joined = format!(" and all {all}");
        for form in [Form::Each, Form::All] {
            let Some(group) = phrase_text(second, form) else {
                continue;
            };
            let sentence = if form == Form::All {
                format!(". Destroy all {group}")
            } else {
                format!(". Destroy each {group}")
            };
            said.extend(
                texts
                    .iter()
                    .filter(|text| text.contains(&sentence))
                    .map(|text| text.replacen(&sentence, &joined, 1)),
            );
        }
    }
    texts.extend(said);
}
