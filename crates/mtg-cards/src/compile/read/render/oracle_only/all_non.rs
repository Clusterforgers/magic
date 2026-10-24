//! A group of "non-" creatures that a change applies to, which a card calls "all": "all non-Zombie creatures get -1/-1
//! until end of turn". The renderers write "non-Zombie creatures get" and "each non-Zombie creature gets".

use super::super::{Form, phrase_text};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref};

/// The verbs a group can take, beside the plural noun.
const VERBS: [&str; 4] = ["get", "gain", "have", "lose"];

/// Adds the wordings that put "all" before the plural of a group with a "non-" quality.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for effect in ability.effects() {
        let Effect::Continuous {
            affected: Ref::Each(selector),
            ..
        } = effect
        else {
            continue;
        };
        if !selector.0.iter().any(|pred| matches!(pred, Pred::Not(_))) {
            continue;
        }
        let Some(plural) = phrase_text(selector, Form::All) else {
            continue;
        };
        let mut found = Vec::new();
        for verb in VERBS {
            let (plain, all) = (
                format!(" {plural} {verb} "),
                format!(" all {plural} {verb} "),
            );
            found.extend(
                texts
                    .iter()
                    .filter(|text| text.contains(&plain))
                    .map(|text| text.replacen(&plain, &all, 1)),
            );
        }
        texts.extend(found);
    }
}
