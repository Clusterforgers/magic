//! r40-9 wordings written from a twin another writer words: "If that artifact is an Equipment, attach it to this
//! creature." after a step on a target artifact (Thieving Skydiver), and "that source's controller" as the player of a
//! step after "Whenever a source deals damage to ~" (Crag Saurian).

use serde_json::{Value, json};

use crate::ability::Ability;

/// Adds the r40-9 wordings of the ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    r409_equipment_attached(ability, texts);
    r409_source_controller(ability, texts);
}

/// "Whenever a source deals damage to ~, that source's controller gains control of ~.": the controller of the source of
/// the damage as a player, written from the twin that names a target opponent instead.
fn r409_source_controller(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { when, targets, .. } = ability else {
        return;
    };
    if !targets.is_empty() || !matches!(when, crate::trigger::TriggerPattern::DealsDamage { .. }) {
        return;
    }
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let dealer = json!({"ControllerOf": "Dealer"});
    let text = value.to_string();
    if text.matches(&dealer.to_string()).count() != 1 {
        return;
    }
    let Ok(replaced) =
        serde_json::from_str::<Value>(&text.replace(&dealer.to_string(), r#"{"Target":0}"#))
    else {
        return;
    };
    value = replaced;
    if let Some(triggered) = value.get_mut("Triggered").and_then(Value::as_object_mut) {
        triggered.insert("targets".to_owned(), json!([{"what": "Opponent"}]));
    }
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return;
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| text.starts_with("Whenever a source deals damage to ~, "))
        .filter(|text| text.matches("target opponent").count() == 1)
        .map(|text| text.replacen("target opponent", "that source's controller", 1))
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// "If that artifact is an Equipment, attach it to ~.": the last step attaches the target artifact to the source when it
/// has the subtype, written from the twin that draws a card instead ("If it is an Equipment, draw a card.").
fn r409_equipment_attached(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(triggered) = value.get_mut("Triggered") else {
        return;
    };
    let artifact = json!({"what": {"Object": [{"Type": "Artifact"}]}});
    let Some(spec) = triggered.pointer("/targets/0/what/Object/0") else {
        return;
    };
    if Some(spec) != artifact.pointer("/what/Object/0") {
        return;
    }
    let Some(Value::Array(effects)) = triggered.get_mut("effects") else {
        return;
    };
    let Some(last) = effects.last_mut() else {
        return;
    };
    let Some(subtype) = last
        .pointer("/If/condition/CountOf/selector/0/Subtype")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };
    let expected = json!({"If": {
        "condition": {"CountOf": {"subject": {"Target": 0}, "selector": [{"Subtype": subtype}], "op": "Ge", "right": 1}},
        "then": [{"Attach": {"what": {"Target": 0}, "to": "This"}}]
    }});
    if *last != expected {
        return;
    }
    if let Some(then) = last.pointer_mut("/If/then") {
        *then = json!([{"Draw": {}}]);
    }
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return;
    };
    let article = if subtype.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    let twin_words = format!("If it is {article} {subtype}, draw a card.");
    let words = format!("If that artifact is {article} {subtype}, attach it to ~.");
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| text.matches(&twin_words).count() == 1)
        .map(|text| text.replacen(&twin_words, &words, 1))
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}
