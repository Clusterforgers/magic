//! A choice of one of two modes that differ in a phrase of the sentence and take no target: "Enchanted creature gets
//! +1/-1 or -1/+1 until end of turn." (Pemmin's Aura). The plain wording is the ability with its first mode only; the
//! sentence of that mode is swapped for the two joined by "or" (`either::joined`).

use super::super::either::{joined, sentences};
use super::super::{capitalize, variants};
use super::twin_json::{inner_mut, twin};
use super::w4b_stand::swapped_in;
use crate::ability::Ability;
use crate::effect::{Effect, Mode};
use serde_json::Value;

/// Adds the wordings of an ability whose one effect is a plain choice of one of two modes without targets.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let [
        Effect::Modal {
            choose,
            repeat: false,
            at_random: false,
            once_each_turn: false,
            once_ever: false,
            all_if: None,
            modes,
        },
    ] = ability.effects()
    else {
        return;
    };
    let [first, second] = modes.as_slice() else {
        return;
    };
    if !choose.is_one() || !first.targets.is_empty() || !second.targets.is_empty() {
        return;
    }
    let Some(plain) = twin(ability, only_the_first) else {
        return;
    };
    let wordings = variants(&plain);
    for (one, other) in pairs(first, second) {
        let Some(either) = joined(&one, &other) else {
            continue;
        };
        texts.extend(swapped_in(&wordings, &[(&one, &either)]));
        texts.extend(swapped_in(
            &wordings,
            &[(&capitalize(&one), &capitalize(&either))],
        ));
    }
}

/// The sentences of the two modes, each with each.
fn pairs(first: &Mode, second: &Mode) -> Vec<(String, String)> {
    let others = sentences(second);
    sentences(first)
        .into_iter()
        .flat_map(|one| others.iter().map(move |other| (one.clone(), other.clone())))
        .collect()
}

/// The ability's JSON with the effects of its first mode in place of the choice.
fn only_the_first(value: &mut Value) -> Option<()> {
    let body = inner_mut(value)?;
    let choice = body.get("effects")?.as_array()?.first()?.get("Modal")?;
    let first = choice
        .get("modes")?
        .as_array()?
        .first()?
        .get("effects")?
        .clone();
    *body.get_mut("effects")? = first;
    Some(())
}
