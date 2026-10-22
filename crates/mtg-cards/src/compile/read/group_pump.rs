//! A pump of a group worded with "each": "Each other creature you control gets +1/+1 until end of turn." The data holds
//! the group; the renderer writes the plural first ("Other creatures you control get +1/+1") and this adds the singular.

use super::render::{capitalize, is_a_type};
use super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// The texts of an ability with the group of a pump worded "each <group> gets" beside "<group> get".
pub(super) fn wordings(ability: &Ability, texts: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for effect in ability.effects() {
        let Effect::Continuous {
            affected: Ref::Each(selector),
            ..
        } = effect
        else {
            continue;
        };
        let (Some(plural), Some(single)) = (
            phrase_text(selector, Form::All),
            phrase_text(selector, Form::Each),
        ) else {
            continue;
        };
        let plural = if selector.0.iter().all(is_a_type) {
            format!("all {plural}")
        } else {
            plural
        };
        let single = format!("each {single}");
        for text in texts {
            for (plain, each) in [(plural.clone(), single.clone()), (capitalize(&plural), capitalize(&single))] {
                for (many, one) in [("get", "gets"), ("gain", "gains")] {
                    let from = format!("{plain} {many} ");
                    if text.contains(&from) {
                        let singular = text
                            .replacen(&from, &format!("{each} {one} "), 1)
                            .replacen(" and gain ", " and gains ", 1);
                        found.push(singular);
                    }
                }
            }
        }
    }
    found
}
