//! Static abilities: an `S:` line as the DSL's `Static`, `Rule` or `CostModifier` ability.

use serde_json::{Value, json};

use super::amount::amount;
use super::condition::{condition, strip};
use super::cost::cost;
use super::ctx::{Ctx, Tr, is_hint, unmapped};
use super::keyword::keyword_list;
use super::valid::{defined, selector_any};
use crate::forge::script::Params;

/// A static line as an ability.
pub fn static_line(ctx: &mut Ctx, body: &str) -> Tr<Value> {
    ctx.reset();
    let all = Params::parse(body);
    let condition = condition(ctx, &all)?;
    let p = strip(&all);
    let mut ability = match p.get("Mode").unwrap_or_default() {
        "Continuous" if p.has("MayPlay") => {
            json!({"Rule": {"rule": super::mayplay::may_cast(ctx, &p, None)?}})
        }
        "Continuous" => continuous(ctx, &p)?,
        "CantAttack" => restriction(ctx, &p, "CantAttack")?,
        "CantBlock" => restriction(ctx, &p, "CantBlock")?,
        "CantBlockBy" => cant_block_by(ctx, &p)?,
        "MustAttack" => must_attack(ctx, &p)?,
        "ReduceCost" => cost_modifier(ctx, &p, "Reduce")?,
        "RaiseCost" => cost_modifier(ctx, &p, "Increase")?,
        other => return unmapped(format!("static:{other}")),
    };
    if condition.is_some() && ability.pointer("/Static/defining").is_some() {
        return unmapped("key:Static.Continuous.defining-with-condition");
    }
    if let Some(condition) = condition
        && let Some(inner) = ability.as_object_mut().and_then(|o| o.values_mut().next())
    {
        inner["condition"] = condition;
    }
    Ok(ability)
}

/// Refuses a key the mode does not read.
fn keys(p: &Params, mode: &str, read: &[&str]) -> Tr<()> {
    for (key, _) in p.iter() {
        if key != "Mode" && !read.contains(&key) && !is_hint(key) {
            return unmapped(format!("key:Static.{mode}.{key}"));
        }
    }
    Ok(())
}

/// The objects a static line affects: `Affected$` or `AffectedDefined$`.
fn affected(ctx: &Ctx, p: &Params) -> Tr<Value> {
    match (p.get("AffectedDefined"), p.get("Affected")) {
        (Some(word), None | Some("Creature" | "Card" | "Permanent")) => static_defined(ctx, word),
        (Some(_), Some(_)) => unmapped("static-affected:Defined+Affected"),
        (None, Some(valid)) => affected_valid(ctx, valid),
        (None, None) if p.get("CharacteristicDefining") == Some("True") => Ok(json!("This")),
        (None, None) => unmapped("static-affected:none"),
    }
}

fn static_defined(ctx: &Ctx, word: &str) -> Tr<Value> {
    match word {
        "Self" | "Enchanted" | "Equipped" => defined(ctx, word),
        other => unmapped(format!("static-affected:{other}")),
    }
}

fn affected_valid(ctx: &Ctx, word: &str) -> Tr<Value> {
    match word {
        "Card.Self" | "Creature.Self" => Ok(json!("This")),
        "Card.EnchantedBy" | "Creature.EnchantedBy" | "Card.EquippedBy" | "Creature.EquippedBy" => {
            Ok(json!("Attached"))
        }
        "You" | "Player" | "Opponent" => unmapped(format!("static-affected:{word}")),
        valid => Ok(json!({"Each": selector_any(ctx, valid)?})),
    }
}

/// The keys a continuous static line reads.
const CONTINUOUS: [&str; 14] = [
    "Affected",
    "AffectedDefined",
    "AddPower",
    "AddToughness",
    "SetPower",
    "SetToughness",
    "AddKeyword",
    "RemoveKeyword",
    "RemoveAllAbilities",
    "EffectZone",
    "CharacteristicDefining",
    "GainControl",
    "AddType",
    "SetColor",
];

fn continuous(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Continuous", &CONTINUOUS)?;
    let affected = affected(ctx, p)?;
    let mut mods = Vec::new();
    if p.has("AddPower") || p.has("AddToughness") {
        let power = amount(ctx, p.get("AddPower").unwrap_or("0"))?;
        let toughness = amount(ctx, p.get("AddToughness").unwrap_or("0"))?;
        mods.push(json!({"ModifyPT": {"power": power, "toughness": toughness}}));
    }
    match (p.get("SetPower"), p.get("SetToughness")) {
        (Some(power), Some(toughness)) => {
            let (power, toughness) = (amount(ctx, power)?, amount(ctx, toughness)?);
            mods.push(json!({"SetPT": {"power": power, "toughness": toughness}}));
        }
        (None, None) => {}
        _ => return unmapped("key:Static.Continuous.half-SetPT"),
    }
    for (key, variant) in [
        ("AddKeyword", "AddKeyword"),
        ("RemoveKeyword", "RemoveKeyword"),
    ] {
        if let Some(keywords) = p.get(key) {
            for keyword in keyword_list(ctx, keywords)? {
                mods.push(json!({ variant: keyword }));
            }
        }
    }
    if p.get("RemoveAllAbilities") == Some("True") {
        mods.push(json!("LoseAbilities"));
    }
    match p.get("GainControl") {
        None => {}
        Some("You") => mods.push(json!({"SetController": "You"})),
        Some(other) => return unmapped(format!("key:Static.Continuous.GainControl={other}")),
    }
    if p.has("AddType") || p.has("SetColor") {
        return unmapped("key:Static.Continuous.type-or-color");
    }
    if mods.is_empty() {
        return unmapped("static:Continuous-nothing");
    }
    let mut ability = json!({"affected": affected, "mods": mods});
    match p.get("EffectZone") {
        None | Some("Battlefield") => {}
        Some(zone @ ("Graveyard" | "Hand" | "Exile" | "Library")) => ability["from"] = json!(zone),
        Some(other) => return unmapped(format!("key:Static.EffectZone={other}")),
    }
    if p.get("CharacteristicDefining") == Some("True") {
        // Only power and toughness are defined this way here, and always, never under a condition.
        if mods
            .iter()
            .any(|modification| modification.get("SetPT").is_none())
        {
            return unmapped("key:Static.Continuous.defining-other-than-stats");
        }
        ability["defining"] = json!(true);
    }
    Ok(json!({ "Static": ability }))
}

/// `CantAttack`, `CantBlock` of the cards named by `ValidCard$`.
fn restriction(ctx: &Ctx, p: &Params, modification: &str) -> Tr<Value> {
    keys(p, modification, &["ValidCard"])?;
    let affected = affected_valid(ctx, p.get("ValidCard").unwrap_or_default())?;
    Ok(json!({"Static": {"affected": affected, "mods": [modification]}}))
}

/// `CantBlockBy`: the attacker named by `ValidAttacker$` is not blocked by the blockers of `ValidBlocker$`, or a blocker
/// that is the card itself blocks only attackers that are not excluded.
fn cant_block_by(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "CantBlockBy", &["ValidAttacker", "ValidBlocker"])?;
    let attacker = p.get("ValidAttacker").unwrap_or_default();
    let blocker = p.get("ValidBlocker");
    match (attacker, blocker) {
        ("Creature.Self" | "Card.Self", None) => {
            Ok(json!({"Static": {"affected": "This", "mods": ["CantBeBlocked"]}}))
        }
        ("Creature.Self" | "Card.Self", Some(valid)) => Ok(json!({"Static": {
            "affected": "This",
            "mods": [{"CantBeBlockedBy": selector_any(ctx, valid)?}],
        }})),
        (valid, Some("Creature.Self" | "Card.Self")) => {
            let mut only = selector_any(ctx, valid)?;
            let (Some(at), 1) = (
                only.iter().position(|pred| pred.get("Not").is_some()),
                only.iter().filter(|pred| pred.get("Not").is_some()).count(),
            ) else {
                return unmapped("static:CantBlockBy-shape");
            };
            let inner = only.remove(at)["Not"].clone();
            only.push(inner);
            Ok(json!({"Static": {"affected": "This", "mods": [{"CanBlockOnly": only}]}}))
        }
        _ => unmapped("static:CantBlockBy-shape"),
    }
}

fn must_attack(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "MustAttack", &["ValidCreature"])?;
    let who = match p.get("ValidCreature").unwrap_or_default() {
        "Card.Self" | "Creature.Self" => json!("This"),
        "Creature.EnchantedBy" => json!("Attached"),
        valid => json!({"Each": selector_any(ctx, valid)?}),
    };
    Ok(json!({"Rule": {"rule": {"MustAttack": who}}}))
}

/// `ReduceCost` and `RaiseCost` of spells.
fn cost_modifier(ctx: &Ctx, p: &Params, change: &str) -> Tr<Value> {
    keys(
        p,
        "CostModifier",
        &["ValidCard", "Type", "Activator", "Amount", "EffectZone"],
    )?;
    if p.get("Type") != Some("Spell") {
        return unmapped("static:CostModifier-Type");
    }
    let applies = match p.get("ValidCard").unwrap_or("Card") {
        "Card.Self" => {
            if p.has("Activator") {
                return unmapped("static:CostModifier-self-with-Activator");
            }
            vec![json!("This")]
        }
        valid => {
            let mut selector = vec![json!("Spell")];
            selector.extend(selector_any(ctx, valid)?);
            match p.get("Activator") {
                None => {}
                Some("You") => selector.push(json!({"ControlledBy": "You"})),
                Some("Player.Opponent" | "Opponent") => {
                    selector.push(json!({"ControlledBy": "Opponents"}));
                }
                Some(other) => return unmapped(format!("static:CostModifier-Activator={other}")),
            }
            selector
        }
    };
    let amount = p.get("Amount").unwrap_or_default();
    // A number or coloured symbols are the mana; a count (an `SVar`) is how many times {1} applies.
    if ctx.svar(amount).is_some() {
        let per = super::amount::amount(ctx, amount)?;
        return Ok(
            json!({"CostModifier": {"applies_to": applies, "change": change, "mana": "{1}", "per": per}}),
        );
    }
    let Value::String(mana) = cost(ctx, amount)? else {
        return unmapped("static:CostModifier-Amount");
    };
    if mana.contains('X') {
        return unmapped("static:CostModifier-X");
    }
    Ok(json!({"CostModifier": {"applies_to": applies, "change": change, "mana": mana}}))
}
