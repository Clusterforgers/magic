//! Abilities: an `A:` line as a spell, an activated ability or a loyalty ability.

use serde_json::{Value, json};

use super::cost::{cost, loyalty};
use super::condition::{condition, strip};
use super::ctx::{Ctx, Tr, unmapped};
use super::effects::chain;
use crate::forge::script::Params;

/// An `A:` line as an ability; `None` for a line the DSL states by other means (an Aura's attach).
pub fn ability_line(ctx: &mut Ctx, body: &str) -> Tr<Option<Value>> {
    ctx.reset();
    let p = Params::parse(body);
    if p.has("SP") {
        if ctx.is_aura() && p.get("SP") == Some("Attach") {
            return Ok(None);
        }
        return spell(ctx, &p).map(Some);
    }
    if p.has("AB") {
        return activated(ctx, &p).map(Some);
    }
    unmapped("line:ability-without-SP-or-AB")
}

/// The keys only an activated ability has.
const ACTIVATION: [&str; 6] = [
    "SorcerySpeed",
    "PlayerTurn",
    "ActivationLimit",
    "ActivationZone",
    "Planeswalker",
    "Ultimate",
];

fn spell(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    if let Some(key) = ACTIVATION.iter().find(|key| p.has(key)) {
        return unmapped(format!("key:Spell.{key}"));
    }
    if let Some(text) = p.get("Cost")
        && !matches!(cost(ctx, text), Ok(Value::String(_)))
    {
        return unmapped("cost:spell-additional");
    }
    let effects = chain(ctx, p)?;
    let mut ability = json!({ "effects": effects });
    if !ctx.targets.is_empty() {
        ability["targets"] = Value::Array(ctx.targets.clone());
    }
    Ok(json!({ "Spell": ability }))
}

fn activated(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let Some(text) = p.get("Cost") else {
        return unmapped("line:AB-without-cost");
    };
    let when = condition(ctx, p)?;
    let p = &strip(p);
    let effects = chain(ctx, p)?;
    let targets = ctx.targets.clone();
    if p.get("Planeswalker") == Some("True") {
        if when.is_some() {
            return unmapped("key:Loyalty.condition");
        }
        return loyalty_ability(p, text, effects, targets);
    }
    let mut ability = json!({"cost": cost(ctx, text)?, "effects": effects});
    let mut conditions: Vec<Value> = when.into_iter().collect();
    if p.get("PlayerTurn") == Some("True") {
        conditions.insert(0, json!("YourTurn"));
    }
    match conditions.len() {
        0 => {}
        1 => ability["condition"] = conditions.remove(0),
        _ => ability["condition"] = json!({ "All": conditions }),
    }
    if p.get("SorcerySpeed") == Some("True") {
        ability["timing"] = json!("Sorcery");
    }
    match p.get("ActivationLimit") {
        None => {}
        Some("1") => ability["limit"] = json!("OncePerTurn"),
        Some(other) => return unmapped(format!("key:AB.ActivationLimit={other}")),
    }
    match p.get("ActivationZone") {
        None | Some("Battlefield") => {}
        Some(zone @ ("Graveyard" | "Hand" | "Exile")) => ability["from"] = json!(zone),
        Some(other) => return unmapped(format!("key:AB.ActivationZone={other}")),
    }
    if !targets.is_empty() {
        ability["targets"] = Value::Array(targets);
    }
    Ok(json!({ "Activated": ability }))
}

fn loyalty_ability(p: &Params, text: &str, effects: Vec<Value>, targets: Vec<Value>) -> Tr<Value> {
    let Some(cost) = loyalty(text) else {
        return unmapped("cost:loyalty-shape");
    };
    if let Some(key) = ["SorcerySpeed", "PlayerTurn", "ActivationLimit", "ActivationZone"]
        .iter()
        .find(|key| p.has(key))
    {
        return unmapped(format!("key:Loyalty.{key}"));
    }
    let mut ability = json!({"cost": cost, "effects": effects});
    if !targets.is_empty() {
        ability["targets"] = Value::Array(targets);
    }
    Ok(json!({ "Loyalty": ability }))
}
