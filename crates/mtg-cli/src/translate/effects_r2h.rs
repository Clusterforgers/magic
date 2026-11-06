//! Effects of the smaller named mechanics: `EachDamage` (each object deals damage as itself) and the rest of the pack.

use mtg_cards::macros;
use mtg_cards::refs::Ref;
use mtg_cards::value::Amount;
use serde_json::{Value, json};

use super::amount::amount;
use super::cost::cost;
use super::ctx::{Ctx, Tr, only, unmapped};
use super::valid::{defined, selector_any, subject};
use crate::forge::script::Params;

/// The effects of a verb this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    Some(match api {
        "EachDamage" => each_damage(ctx, p),
        "Empower" => empower(ctx, p),
        "Endure" => endure(ctx, p),
        "Detain" => detain(ctx, p),
        _ => return None,
    })
}

/// `Detain`: until the controller's next turn the objects can't attack or block and their abilities can't be activated
/// (mana abilities stay, as `Prohibited::Activate` leaves them).
fn detain(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Detain",
        &["Defined", "ValidTgts", "TargetMin", "TargetMax"],
    )?;
    let what = subject(ctx, p, "Self", None)?;
    Ok(vec![
        json!({"Continuous": {"affected": what, "mods": ["CantAttack", "CantBlock"],
            "duration": "UntilYourNextTurn"}}),
        json!({"Rule": {"rule": {"Cant": {"who": "EachPlayer",
            "action": {"Activate": [{"Is": what}]}}}, "duration": "UntilYourNextTurn"}}),
    ])
}

/// A keyword action of the cards crate (`empower`, `endure`) on the object, with the number the line names.
fn action(ctx: &mut Ctx, p: &Params, name: &str, on: &Value) -> Tr<Vec<Value>> {
    let Some(num) = p.get("Num") else {
        return unmapped(format!("key:{name}.no-Num"));
    };
    let count: Amount = from_value(amount(ctx, num)?, name)?;
    let on: Ref = from_value(on.clone(), name)?;
    let Some(effects) = macros::action_effects_of(name, &count, &on) else {
        return unmapped(format!("effect:{name}"));
    };
    effects
        .iter()
        .map(|effect| serde_json::to_value(effect).or_else(|_| unmapped("effect:serialize")))
        .collect()
}

fn from_value<T: serde::de::DeserializeOwned>(value: Value, what: &str) -> Tr<T> {
    serde_json::from_value(value).or_else(|_| unmapped(format!("effect:{what}-value")))
}

/// `Empower`: loyalty counters on the controller's Jace token, made first when there is none.
fn empower(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "Empower", &["Type", "Num"])?;
    if p.get("Type") != Some("Jace") {
        return unmapped("key:Empower.Type");
    }
    action(ctx, p, "empower", &json!("This"))
}

/// `Endure`: the object gets the counters, or its controller gets a Spirit token of that size.
fn endure(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "Endure", &["Num", "Defined"])?;
    let on = defined(ctx, p.get("Defined").unwrap_or("Self"))?;
    action(ctx, p, "endure", &on)
}

/// The keyword lines of the shapes `Kicker:<cost>:<cost>` (two kickers), `Equip:<cost>:<valid>:<words>` (equip to a kind of
/// creature) and `Awaken:<n>:<cost>`; `None` for any other line.
pub fn keyword_forms(ctx: &mut Ctx, body: &str) -> Option<Vec<Tr<Value>>> {
    let fields: Vec<&str> = body.split(':').collect();
    match fields.as_slice() {
        ["Kicker", first, second] if cost(ctx, second).is_ok() => Some(
            [first, second]
                .iter()
                .map(|text| {
                    let line = format!("Kicker:{text}");
                    super::card::audited_one(|| {
                        Ok(json!({"Keyword": super::keyword::keyword_value(ctx, &line)?}))
                    })
                })
                .collect(),
        ),
        ["Equip", price, valid, _words] if !valid.is_empty() && !valid.contains('$') => {
            Some(vec![equip_to(ctx, price, valid)])
        }
        ["Reinforce", count, price] => Some(vec![reinforce(ctx, count, price)]),
        ["Awaken", count, price] => {
            let count: u32 = count.parse().ok()?;
            Some(vec![cost(ctx, price).map(
                |cost| json!({"Keyword": {"Awaken": {"count": count, "cost": cost}}}),
            )])
        }
        _ => None,
    }
}

/// `Reinforce N` (CR 702.77 of the old rules): from the hand, for the cost and the discard of the card, N +1/+1 counters on a
/// target creature. `Reinforce X` takes the X the cost names.
fn reinforce(ctx: &mut Ctx, count: &str, price: &str) -> Tr<Value> {
    let count = if count == "X" {
        json!("X")
    } else {
        json!(
            count
                .parse::<u32>()
                .or_else(|_| unmapped("keyword:Reinforce-shape"))?
        )
    };
    let mut parts = match cost(ctx, price)? {
        Value::Array(parts) => parts,
        other => vec![json!({ "Mana": other })],
    };
    parts.push(json!("DiscardSelf"));
    Ok(json!({"Activated": {
        "cost": parts,
        "from": "Hand",
        "targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"PutCounters": {"on": {"Target": 0}, "counter": "+1/+1", "count": count}}]}}))
}

/// `Equip` to a kind of creature: the creature must also match the valid-string, which has to name a creature.
fn equip_to(ctx: &mut Ctx, price: &str, valid: &str) -> Tr<Value> {
    let kind = valid.split(['.', '+']).next().unwrap_or_default();
    if kind != "Creature" && !ctx.world.subtypes.contains(kind) {
        return unmapped("keyword:Equip-to-a-noncreature");
    }
    let to = selector_any(ctx, valid)?;
    Ok(json!({"Keyword": {"EquipTo": {"to": to, "cost": cost(ctx, price)?}}}))
}

/// `K:ETBReplacement:Other:<part>:Mandatory:<zone>:<valid>` whose part puts counters on the replaced card: every object the
/// valid-string selects enters with that many more counters, while the source is on the battlefield or in the graveyard.
pub fn extra_counters(ctx: &mut Ctx, name: &str, zone: &str, valid: &str) -> Tr<Value> {
    let Some(text) = ctx.svar(name) else {
        return unmapped("svar:missing");
    };
    let part = Params::parse(text);
    let exact = part.get("DB") == Some("PutCounter")
        && part.get("ETB") == Some("True")
        && part.get("Defined") == Some("ReplacedCard");
    if !exact {
        return unmapped("keyword:ETBReplacement-shape");
    }
    super::ctx::check_keys(
        &part,
        "ETBReplacement",
        &["DB", "ETB", "Defined", "CounterType", "CounterNum"],
        |_, _| false,
    )?;
    let Some(kind) = part
        .get("CounterType")
        .and_then(super::amount::counter_kind)
    else {
        return unmapped("keyword:ETBReplacement-counter-kind");
    };
    let count = amount(ctx, part.get("CounterNum").unwrap_or("1"))?;
    let mut replacement = json!({
        "when": {"Enters": {"what": selector_any(ctx, valid)?}},
        "by": {"WithCounters": {"counter": kind, "count": count}}});
    match zone {
        "Battlefield" => {}
        "Graveyard" => replacement["from"] = json!("Graveyard"),
        other => return unmapped(format!("keyword:ETBReplacement-zone={other}")),
    }
    Ok(json!({ "Replacement": replacement }))
}

/// The number each damager deals: its own power or toughness (`Count$CardPower`), or a number that does not depend on it.
fn damage_of(ctx: &mut Ctx, p: &Params, each: &Value) -> Tr<Value> {
    let mut word = p.get("NumDmg").unwrap_or("1");
    if let Some(inner) = ctx.svars.get(word) {
        word = inner;
    }
    let stat = match word.strip_prefix("Count$") {
        Some("CardPower") => Some("Power"),
        Some("CardToughness") => Some("Toughness"),
        _ => None,
    };
    if let Some(stat) = stat {
        return Ok(json!({"Stat": {"of": each, "stat": stat}}));
    }
    let node = amount(ctx, word)?;
    if node.to_string().contains("\"This\"") {
        return unmapped("amount:EachDamage-of-the-source");
    }
    Ok(node)
}

/// `EachDamage`: every object the line names deals damage as itself, the amount read from that object. Objects that deal
/// damage to each other (`ToEachOther$ Targeted`) are the two targets of the ability.
fn each_damage(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "EachDamage",
        &[
            "ValidCards",
            "DefinedDamagers",
            "NumDmg",
            "Defined",
            "ValidTgts",
            "EachToItself",
            "ToEachOther",
            "TargetMin",
            "TargetMax",
        ],
    )?;
    if p.get("ToEachOther") == Some("Targeted") && p.has("ValidTgts") {
        let first = ctx.parent()?;
        let second = subject(ctx, p, "Targeted", None)?;
        let (one, other) = (damage_of(ctx, p, &first)?, damage_of(ctx, p, &second)?);
        return Ok(vec![
            json!({"DealDamage": {"source": first, "to": second, "amount": one}}),
            json!({"DealDamage": {"source": second, "to": first, "amount": other}}),
        ]);
    }
    let over = match (p.get("ValidCards"), p.get("DefinedDamagers")) {
        (Some(valid), None) => json!({"Each": selector_any(ctx, valid)?}),
        (None, Some(word)) => defined(ctx, word)?,
        _ => return unmapped("key:EachDamage.damagers"),
    };
    let name = ctx.fresh("damager");
    let each = json!({ "Bound": name });
    let to = if p.get("EachToItself") == Some("True") {
        each.clone()
    } else if p.has("Defined") || p.has("ValidTgts") {
        subject(ctx, p, "Self", None)?
    } else {
        return unmapped("key:EachDamage.recipient");
    };
    let amount = damage_of(ctx, p, &each)?;
    Ok(vec![
        json!({"RepeatEach": {"over": over, "as": name, "effects": [
        {"DealDamage": {"source": each, "to": to, "amount": amount}}]}}),
    ])
}
