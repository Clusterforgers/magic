//! A return to the battlefield whose card enters with counters when it is of a kind: "Return target creature card from your
//! graveyard to the battlefield. If a Hero enters this way, it enters with two additional +1/+1 counters on it." The data
//! is a choice on the target's kind between the move with the counters and the plain move; the ability is written with the
//! plain move and the sentence about the counters after it.

use serde_json::{Value, json};

use super::amounts::count_word;
use crate::ability::Ability;

/// The ability with the plain move in place of the choice, and the sentences about the counters; `None` when the last
/// effect of the ability (or of its last mode) is not that choice.
pub fn plain(ability: &Ability) -> Option<(Ability, Vec<String>)> {
    let mut value = serde_json::to_value(ability).ok()?;
    let effects = value
        .as_object_mut()?
        .values_mut()
        .next()?
        .get_mut("effects")?;
    let last = last_list(effects)?;
    let choice = last.last_mut()?;
    let (plain, sentences) = split(choice)?;
    *choice = plain;
    Some((serde_json::from_value(value).ok()?, sentences))
}

/// The effect list that ends the ability: the effects themselves, or those of the last mode when the one effect is modal.
fn last_list(effects: &mut Value) -> Option<&mut Vec<Value>> {
    let is_modal = effects
        .as_array()
        .is_some_and(|list| list.len() == 1 && list[0].get("Modal").is_some());
    if is_modal {
        return effects[0]["Modal"]["modes"]
            .as_array_mut()?
            .last_mut()?
            .get_mut("effects")?
            .as_array_mut();
    }
    effects.as_array_mut()
}

/// The plain move and the sentences of a choice between the two moves of one target.
fn split(choice: &Value) -> Option<(Value, Vec<String>)> {
    let branch = choice.get("If")?;
    (branch.as_object()?.len() == 3).then_some(())?;
    let count = branch["condition"].get("CountOf")?;
    let target = &count["subject"];
    (target.get("Target").is_some()
        && count.as_object()?.len() == 4
        && count["op"] == json!("Ge")
        && count["right"] == json!(1))
    .then_some(())?;
    let kind = kind_text(count["selector"].as_array()?)?;
    let [plain] = branch["otherwise"].as_array()?.as_slice() else {
        return None;
    };
    let [with] = branch["then"].as_array()?.as_slice() else {
        return None;
    };
    (*plain == json!({"Move": {"to": {"Battlefield": {}}, "what": target}})).then_some(())?;
    let [entry] = with["Move"]["to"]["Battlefield"]["counters"]
        .as_array()?
        .as_slice()
    else {
        return None;
    };
    (*with == json!({"Move": {"to": {"Battlefield": {"counters": [entry]}}, "what": target}}))
        .then_some(())?;
    let counters = counters_text(entry)?;
    let article = super::triggers::article(&kind);
    Some((
        plain.clone(),
        vec![
            format!("If {article} {kind} enters this way, it enters with {counters} on it."),
            format!("It enters with {counters} on it if it's {article} {kind}."),
        ],
    ))
}

/// "Hero", "creature": the one kind the choice tests.
fn kind_text(selector: &[Value]) -> Option<String> {
    let [pred] = selector else {
        return None;
    };
    if let Some(kind) = pred.get("Type").and_then(Value::as_str) {
        return Some(kind.to_lowercase());
    }
    pred.get("Subtype")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// "an additional +1/+1 counter", "two additional +1/+1 counters".
fn counters_text(entry: &Value) -> Option<String> {
    let [kind, number] = entry.as_array()?.as_slice() else {
        return None;
    };
    (*kind == json!("+1/+1")).then_some(())?;
    let number = u32::try_from(number.as_u64()?).ok()?;
    Some(if number == 1 {
        "an additional +1/+1 counter".to_owned()
    } else {
        format!("{} additional +1/+1 counters", count_word(number)?)
    })
}
