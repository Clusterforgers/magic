//! "When this creature becomes monstrous, create X X/X green Hydra creature tokens": X in such a trigger is the X of
//! the monstrosity (CR 701.37c), the amount of the event (`Amount::Triggering`).

use serde_json::Value;

use super::render;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::trigger::TriggerPattern;
use crate::value::Amount;

/// Lets X read in the effects of a trigger on becoming monstrous; the earlier `x_free`, which `c10_pa_put_in` restores.
pub(super) fn c10_pa_x_of(r: &mut Reader<'_>, when: &TriggerPattern) -> Option<bool> {
    matches!(when, TriggerPattern::BecomesMonstrous { .. })
        .then(|| std::mem::replace(&mut r.x_free, true))
}

/// The effects with the X of the monstrosity in the place of each X.
pub(super) fn c10_pa_put_in(
    r: &mut Reader<'_>,
    effects: Vec<Effect>,
    before: Option<bool>,
) -> Vec<Effect> {
    let Some(before) = before else {
        return effects;
    };
    r.x_free = before;
    c10_pa_swap(&effects, &Amount::X, &Amount::Triggering).unwrap_or(effects)
}

/// The wordings of a trigger on becoming monstrous with the amount of the event written as X.
pub(super) fn c10_pa_wordings(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when: TriggerPattern::BecomesMonstrous { .. },
        effects,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let Some(with_x) = c10_pa_swap(effects, &Amount::Triggering, &Amount::X) else {
        return Vec::new();
    };
    let mut replaced = ability.clone();
    if let Ability::Triggered { effects, .. } = &mut replaced {
        *effects = with_x;
    }
    render::variants(&replaced)
}

/// The effects with `into` in the place of each `from`; `None` when there is none.
pub(super) fn c10_pa_swap(effects: &[Effect], from: &Amount, into: &Amount) -> Option<Vec<Effect>> {
    let mut json = serde_json::to_value(effects).ok()?;
    let from = serde_json::to_value(from).ok()?;
    let into = serde_json::to_value(into).ok()?;
    c10_pa_replace(&mut json, &from, &into).then_some(())?;
    serde_json::from_value(json).ok()
}

fn c10_pa_replace(value: &mut Value, from: &Value, into: &Value) -> bool {
    if value == from {
        value.clone_from(into);
        return true;
    }
    match value {
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |found, v| c10_pa_replace(v, from, into) | found),
        Value::Object(map) => map
            .values_mut()
            .fold(false, |found, v| c10_pa_replace(v, from, into) | found),
        _ => false,
    }
}
