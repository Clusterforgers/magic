//! Cost changes for spells and abilities that target: "Spells your opponents cast that target a Merfolk you control cost
//! {2} more to cast." and "Abilities your opponents activate that target ... cost {2} more to activate.". The sentence
//! is the one of the spells without the target, with the target's phrase after the caster.

use super::super::super::selectors::{Form, phrase_text};
use super::super::super::triggers::article;
use super::super::written_variants;
use crate::ability::Ability;
use crate::refs::{AbilityKind, Pred, Selector};

/// Adds the wordings of a cost change for spells or activated abilities that target a kind of object.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::CostModifier { applies_to, .. } = ability else {
        return;
    };
    let mut targeted = None;
    let mut activated = false;
    let mut rest = Vec::new();
    for pred in &applies_to.0 {
        match pred {
            Pred::Targeting(selector) if targeted.is_none() => targeted = Some(selector),
            Pred::Ability(AbilityKind::Activated) if !activated => activated = true,
            other => rest.push(other.clone()),
        }
    }
    let Some(phrase) = targeted.and_then(|selector| phrase_text(selector, Form::Subject)) else {
        return;
    };
    let mut spells = vec![Pred::Spell];
    spells.extend(rest.into_iter().filter(|pred| *pred != Pred::Spell));
    let mut twin = ability.clone();
    if let Ability::CostModifier { applies_to, .. } = &mut twin {
        *applies_to = Selector(spells);
    }
    let that = format!(" that target {} {phrase} cost ", article(&phrase));
    for text in written_variants(&twin) {
        let Some((head, tail)) = text.split_once(" cast cost ") else {
            continue;
        };
        if !head.starts_with("Spells ") || !tail.ends_with(" to cast.") {
            continue;
        }
        if activated {
            let head = head.replacen("Spells ", "Abilities ", 1);
            let tail = tail.replacen(" to cast.", " to activate.", 1);
            texts.push(format!("{head} activate{that}{tail}"));
        } else {
            texts.push(format!("{head} cast{that}{tail}"));
        }
    }
}
