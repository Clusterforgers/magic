//! A card that returns from its owner's graveyard tapped and attacking as its controller attacks with a group:
//! "Whenever you attack with your commander, if this card is in your graveyard, you may return it to the battlefield
//! tapped and attacking." and "..., you may pay {2}{B}. If you do, return this card from your graveyard ...".

use crate::ability::Ability;
use serde_json::{Value, json};

/// The groups an attack names, as the selector's JSON, each with its words.
const C6_GL_GROUPS: [(&str, &str); 3] = [
    (
        r#"["Commander",{"OwnedBy":"You"},{"ControlledBy":"You"}]"#,
        "your commander",
    ),
    (
        r#"[{"Supertype":"Legendary"},{"Type":"Creature"},{"ControlledBy":"You"}]"#,
        "one or more legendary creatures",
    ),
    (
        r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#,
        "one or more creatures",
    ),
];

/// Adds the wordings of a return from the graveyard into an attack, bought with a choice or a payment.
pub(super) fn c6_gl_add_returns(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(data) = serde_json::to_value(ability) else {
        return;
    };
    let returns = json!([
        {"Move": {"what": "This", "to": {"Battlefield": {"tapped": true}}}},
        {"JoinAttack": {"what": "This"}}
    ]);
    let Some(step) = data.pointer("/Triggered/effects/0") else {
        return;
    };
    let tail = if *step == c6_gl_value(json!({"May": {"effects": returns}})) {
        "if this card is in your graveyard, you may return it to the battlefield tapped and attacking.".to_owned()
    } else if let Some(mana) = c6_gl_paid_mana(step, &returns) {
        format!(
            "you may pay {mana}. If you do, return this card from your graveyard to the battlefield tapped and attacking."
        )
    } else {
        return;
    };
    for (group, words) in C6_GL_GROUPS {
        let Ok(what) = serde_json::from_str::<Value>(group) else {
            continue;
        };
        let head = json!({"Triggered": {
            "when": {"GroupAttacks": {"what": what}},
            "from": "Graveyard",
            "effects": [step.clone()]
        }});
        if c6_gl_value(head) == data {
            texts.push(format!("Whenever you attack with {words}, {tail}"));
        }
    }
}

/// The mana of a payment whose paid steps are the return, as printed.
fn c6_gl_paid_mana(step: &Value, returns: &Value) -> Option<String> {
    let pay = step.get("Pay")?;
    let mana = pay.get("cost")?.as_str()?.to_owned();
    let plain = c6_gl_value(json!({"Pay": {"cost": mana, "paid": returns}}));
    (plain == *step).then_some(mana)
}

/// The JSON as the data writes it, defaults filled in; `Null` when it is no ability or effect.
fn c6_gl_value(value: Value) -> Value {
    if let Ok(ability) = serde_json::from_value::<Ability>(value.clone()) {
        return serde_json::to_value(ability).unwrap_or(Value::Null);
    }
    serde_json::from_value::<crate::effect::Effect>(value)
        .ok()
        .and_then(|effect| serde_json::to_value(effect).ok())
        .unwrap_or(Value::Null)
}
