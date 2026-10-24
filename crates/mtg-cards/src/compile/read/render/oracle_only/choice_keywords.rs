//! A keyword the controller chooses: "{G}: This creature gains your choice of reach, trample, or haste until end of
//! turn." The data is a choice of one mode for each keyword; the renderers write the modes as a list.

use super::super::variants;
use super::bare::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::keyword::Keyword;
use crate::modify::Modification;

/// A keyword as the card words it: "reach", "first strike".
fn word(keyword: &Keyword) -> Option<String> {
    let name = format!("{keyword:?}");
    name.chars().all(char::is_alphabetic).then(|| {
        let mut out = String::new();
        for letter in name.chars() {
            if letter.is_uppercase() && !out.is_empty() {
                out.push(' ');
            }
            out.extend(letter.to_lowercase());
        }
        out
    })
}

/// Adds the wordings of an ability whose only step is a choice among keywords the source gains for a while.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let [
        Effect::Modal {
            choose,
            repeat: false,
            at_random: false,
            once_each_turn: false,
            all_if: None,
            modes,
        },
    ] = ability.effects()
    else {
        return;
    };
    if !choose.is_one() || modes.len() < 2 {
        return;
    }
    let mut words = Vec::new();
    let mut steps = Vec::new();
    for mode in modes {
        let [step @ Effect::Continuous { mods, .. }] = mode.effects.as_slice() else {
            return;
        };
        let [Modification::AddKeyword(keyword)] = mods.as_slice() else {
            return;
        };
        let Some(found) = word(keyword) else {
            return;
        };
        words.push(found);
        steps.push(step.clone());
    }
    let same = steps.iter().all(|step| match (step, &steps[0]) {
        (
            Effect::Continuous {
                affected, duration, ..
            },
            Effect::Continuous {
                affected: first,
                duration: first_duration,
                ..
            },
        ) => affected == first && duration == first_duration,
        _ => false,
    });
    if !mode_targets_empty(modes) || !same {
        return;
    }
    let Some(plain) = with_effects(ability, vec![steps.remove(0)]) else {
        return;
    };
    let Some((last, rest)) = words.split_last() else {
        return;
    };
    let choice = format!("your choice of {}, or {last}", rest.join(", "));
    let one = format!("gains {} ", words[0]);
    for text in variants(&plain) {
        if text.contains(&one) {
            texts.push(text.replacen(&one, &format!("gains {choice} "), 1));
        }
    }
}

/// Whether no mode has a target of its own.
fn mode_targets_empty(modes: &[crate::effect::Mode]) -> bool {
    modes.iter().all(|mode| mode.targets.is_empty())
}
