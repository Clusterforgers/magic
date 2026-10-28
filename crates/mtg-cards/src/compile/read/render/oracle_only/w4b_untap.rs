//! Permanents that don't untap during their controllers' next untap step, which the renderers word only after a tap
//! of one object: the permanent of the event ("Whenever an opponent taps a land for mana, that land doesn't untap
//! during its controller's next untap step.") and every permanent of a group tapped just before ("tap all nonblue
//! creatures. Those creatures don't untap during their controllers' next untap steps.").

use super::super::written_variants;
use super::standin::with_effects;
use super::w4b_stand::{json, stand};
use crate::ability::Ability;
use serde_json::{Value, json};

/// Adds the wordings of an ability whose last effect keeps permanents tapped.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let value = json(ability);
    let Some(effects) = value
        .pointer(&format!("/{}/effects", head(&value)))
        .and_then(Value::as_array)
    else {
        return;
    };
    let Some((last, before)) = effects.split_last() else {
        return;
    };
    let Some(affected) = kept_tapped(last) else {
        return;
    };
    if affected == "Triggering" && before.is_empty() {
        if let Some(noun) = event_noun(&value) {
            let sentence =
                format!("that {noun} doesn't untap during its controller's next untap step");
            texts.extend(with_effects(ability, &sentence));
        }
        return;
    }
    let Some(noun) = group_noun(affected) else {
        return;
    };
    if before.last() != Some(&json!({"Tap": {"what": affected}})) {
        return;
    }
    let Some(tapped) = stand(ability, |value| {
        let key = head(value);
        value
            .pointer_mut(&format!("/{key}/effects"))?
            .as_array_mut()?
            .pop()?;
        Some(())
    }) else {
        return;
    };
    let sentence =
        format!(" Those {noun}s don't untap during their controllers' next untap steps.");
    let written: Vec<String> = written_variants(&tapped)
        .into_iter()
        .filter(|text| text.ends_with('.'))
        .map(|text| format!("{text}{sentence}"))
        .collect();
    texts.extend(written);
}

/// The variant name of the ability.
fn head(value: &Value) -> String {
    value
        .as_object()
        .and_then(|map| map.keys().next().cloned())
        .unwrap_or_default()
}

/// What a `Continuous` effect keeps from untapping during its controller's next untap step.
fn kept_tapped(effect: &Value) -> Option<&Value> {
    let continuous = effect.get("Continuous")?;
    (continuous.get("mods")? == &json!(["DoesntUntap"])
        && continuous.get("duration")? == "NextUntapStep")
        .then_some(())?;
    continuous.get("affected")
}

/// The card type, in lower case, of the permanents of the event ("land" for "an opponent taps a land").
fn event_noun(value: &Value) -> Option<String> {
    let when = value.pointer("/Triggered/when")?.as_object()?;
    let (_, event) = when.iter().next()?;
    match event.get("what")?.as_array()?.as_slice() {
        [only] => Some(only.get("Type")?.as_str()?.to_lowercase()),
        _ => None,
    }
}

/// The card type, in lower case, of a group of permanents ("creature" for all nonblue creatures).
fn group_noun(affected: &Value) -> Option<String> {
    let types: Vec<&str> = affected
        .get("Each")?
        .as_array()?
        .iter()
        .filter_map(|pred| pred.get("Type").and_then(Value::as_str))
        .collect();
    match types.as_slice() {
        [only] => Some(only.to_lowercase()),
        _ => None,
    }
}
