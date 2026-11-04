//! `K:Class:<level>:<cost>:Add<Kind>$ <part>`: the activated ability that raises a Class to the level, and the ability it
//! adds, which is off below that level.

use serde_json::{Value, json};

use super::cost::cost;
use super::ctx::{Ctx, Tr, unmapped};

/// The abilities of a `Class` keyword line, `None` for any other line.
pub fn class_level(ctx: &mut Ctx, body: &str) -> Option<Vec<Tr<Value>>> {
    let rest = body.strip_prefix("Class:")?;
    let fields: Vec<&str> = rest.splitn(3, ':').collect();
    let [level, price, added] = fields.as_slice() else {
        return Some(vec![unmapped("keyword:Class-shape")]);
    };
    let Ok(level) = level.parse::<u32>() else {
        return Some(vec![unmapped("keyword:Class-level")]);
    };
    if level < 2 {
        return Some(vec![unmapped("keyword:Class-level")]);
    }
    Some(vec![
        level_up(ctx, level, price),
        super::card::audited_one(|| added_ability(ctx, level, added)),
    ])
}

/// "{cost}: Level N": a sorcery-speed ability of a Class that is at the level before.
fn level_up(ctx: &Ctx, level: u32, price: &str) -> Tr<Value> {
    Ok(json!({"Activated": {
        "cost": cost(ctx, price)?,
        "timing": "Sorcery",
        "condition": at_level("Eq", level - 1),
        "effects": [{"BecomeLevel": {"level": level}}]}}))
}

fn at_level(op: &str, level: u32) -> Value {
    json!({"Level": {"of": "This", "op": op, "level": level}})
}

/// The ability the level adds, from the part the `Add...$` key names, with the condition that the Class has the level.
fn added_ability(ctx: &mut Ctx, level: u32, added: &str) -> Tr<Value> {
    let Some((kind, name)) = added.split_once("$ ") else {
        return unmapped("keyword:Class-added");
    };
    let text = ctx.svar(name.trim()).map(str::to_owned);
    let Some(text) = text else {
        return unmapped("svar:missing");
    };
    let mut ability = match kind {
        "AddTrigger" => super::triggers::triggered(ctx, &text)?,
        "AddStaticAbility" => super::statics::static_line(ctx, &text)?,
        "AddReplacementEffect" => super::replacements::replacement_line(ctx, &text)?,
        other => return unmapped(format!("keyword:Class-{other}")),
    };
    let key = if kind == "AddTrigger" {
        "if"
    } else {
        "condition"
    };
    let Some(inner) = ability
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
        .and_then(Value::as_object_mut)
    else {
        return unmapped("keyword:Class-shape");
    };
    let held = at_level("Ge", level);
    let condition = match inner.remove(key) {
        Some(existing) => json!({"All": [held, existing]}),
        None => held,
    };
    inner.insert(key.to_owned(), condition);
    Ok(ability)
}
