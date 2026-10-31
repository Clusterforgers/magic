//! Abilities: an `A:` line as a spell, an activated ability or a loyalty ability.

use serde_json::{Value, json};

use super::cost::{cost, loyalty};
use super::condition::{condition, strip};
use super::ctx::{Ctx, Tr, unmapped};
use super::effects::chain;
use crate::forge::script::Params;

/// The abilities of an `A:` line: none for a line the DSL states by other means (an Aura's attach), one, or a spell with
/// the additional cost its `Cost$` states.
pub fn ability_line(ctx: &mut Ctx, body: &str) -> Tr<Vec<Value>> {
    ctx.reset();
    let p = Params::parse(body);
    if p.has("SP") {
        if ctx.is_aura() && p.get("SP") == Some("Attach") {
            return Ok(Vec::new());
        }
        return spell(ctx, &p);
    }
    if p.has("AB") {
        return activated(ctx, &p).map(|ability| vec![ability]);
    }
    unmapped("line:ability-without-SP-or-AB")
}

/// The cost a spell pays besides its mana cost, as the `AdditionalCost` ability that states it. The mana of the cost must
/// be the card's own: another amount is an alternative cost.
fn additional_cost(ctx: &Ctx, text: &str) -> Tr<Option<Value>> {
    let words = super::cost::parts(text);
    let mana: Vec<&str> = words.iter().copied().filter(|w| super::cost::is_mana(w)).collect();
    let printed: Vec<&str> = ctx.mana_cost.split_whitespace().collect();
    if !mana.is_empty() && mana != printed {
        return unmapped("cost:spell-different-mana");
    }
    let Value::Array(items) = cost(ctx, text)? else {
        return Ok(None);
    };
    let items: Vec<Value> = items.into_iter().filter(|item| item.get("Mana").is_none()).collect();
    if items.is_empty() {
        return Ok(None);
    }
    Ok(Some(json!({"AdditionalCost": {"cost": items, "required": true}})))
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

fn spell(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    if let Some(key) = ACTIVATION.iter().find(|key| p.has(key)) {
        return unmapped(format!("key:Spell.{key}"));
    }
    let extra = match p.get("Cost") {
        Some(text) => additional_cost(ctx, text)?,
        None => None,
    };
    let effects = chain(ctx, p)?;
    let mut ability = json!({ "effects": effects });
    if !ctx.targets.is_empty() {
        ability["targets"] = Value::Array(ctx.targets.clone());
    }
    let mut out: Vec<Value> = extra.into_iter().collect();
    out.push(json!({ "Spell": ability }));
    Ok(out)
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
