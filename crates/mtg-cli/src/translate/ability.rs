//! Abilities: an `A:` line as a spell, an activated ability or a loyalty ability.

use serde_json::{Value, json};

use super::condition::{condition, strip};
use super::cost::{cost, loyalty};
use super::ctx::{Ctx, Tr, unmapped};
use super::effects::chain;
use super::header::{ACTIVATION_WORDS, activation_words};
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
    let mana: Vec<&str> = words
        .iter()
        .copied()
        .filter(|w| super::cost::is_mana(w))
        .collect();
    let printed: Vec<&str> = ctx.mana_cost.split_whitespace().collect();
    if !mana.is_empty() && mana != printed {
        return unmapped("cost:spell-different-mana");
    }
    let Value::Array(items) = cost(ctx, text)? else {
        return Ok(None);
    };
    let items: Vec<Value> = items
        .into_iter()
        .filter(|item| item.get("Mana").is_none())
        .collect();
    if items.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        json!({"AdditionalCost": {"cost": items, "required": true}}),
    ))
}

/// The keys only an activated ability has.
const ACTIVATION: [&str; 7] = [
    "SorcerySpeed",
    "ActivationLimit",
    "GameActivationLimit",
    "Exhaust",
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
    // "Cast this spell only during your turn / only during combat" (CR 601.3).
    let mut when: Vec<Value> = Vec::new();
    match p.get("PlayerTurn") {
        None => {}
        Some("True") => when.push(json!("YourTurn")),
        Some(other) => return unmapped(format!("key:Spell.PlayerTurn={other}")),
    }
    let words = activation_words(p)?;
    if words.once_per_turn || words.from.is_some() {
        return unmapped("key:Spell.Boast-or-Forecast");
    }
    when.extend(words.conditions);
    when.extend(super::condition::check_svar(ctx, p)?);
    let only = match when.len() {
        0 => None,
        1 => when.pop(),
        _ => Some(json!({ "All": when })),
    };
    let p = &p
        .without(&ACTIVATION_WORDS)
        .without(&["PlayerTurn", "CheckSVar", "SVarCompare"]);
    let effects = chain(ctx, p)?;
    let mut ability = json!({ "effects": effects });
    if !ctx.targets.is_empty() {
        ability["targets"] = Value::Array(ctx.targets.clone());
    }
    let mut out: Vec<Value> = extra.into_iter().collect();
    out.extend(only.map(|condition| json!({"CastOnly": {"condition": condition}})));
    out.push(json!({ "Spell": ability }));
    Ok(out)
}

fn activated(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let Some(text) = p.get("Cost") else {
        return unmapped("line:AB-without-cost");
    };
    // "If this is tapped / on the battlefield" is checked as the ability resolves; a condition on activation differs. Only
    // a line that says "Activate only if" is one.
    if p.get("IsPresent")
        .is_some_and(|valid| valid.starts_with("Card.Self"))
        && !p
            .get("SpellDescription")
            .is_some_and(|text| text.contains("Activate only if"))
    {
        return unmapped("key:AB.IsPresent=Self");
    }
    let when = condition(ctx, p)?;
    let p = &strip(p);
    let words = activation_words(p)?;
    let p = &p.without(&ACTIVATION_WORDS);
    let game_limited = once_a_game(p)?;
    let by = activator_word(p)?;
    let p = &p.without(&["Exhaust", "GameActivationLimit"]);
    let effects = chain(ctx, &p.without(&["ReduceCost", "Activator"]))?;
    let targets = ctx.targets.clone();
    if p.get("Planeswalker") == Some("True") {
        if when.is_some() || !words.conditions.is_empty() {
            return unmapped("key:Loyalty.condition");
        }
        return loyalty_ability(p, text, &effects, &targets);
    }
    let mut cost_items = cost(ctx, text)?;
    if let Some(less) = reduce_cost(ctx, p)? {
        cost_items = match cost_items {
            Value::String(mana) => json!([{ "Mana": mana }, less]),
            Value::Array(mut items) => {
                items.push(less);
                Value::Array(items)
            }
            _ => return unmapped("key:AB.ReduceCost.cost-shape"),
        };
    }
    let mut ability = json!({"cost": cost_items, "effects": effects});
    let mut conditions: Vec<Value> = when.into_iter().collect();
    if p.get("PlayerTurn") == Some("True") {
        conditions.insert(0, json!("YourTurn"));
    }
    for condition in words.conditions {
        if !conditions.contains(&condition) {
            conditions.push(condition);
        }
    }
    match conditions.len() {
        0 => {}
        1 => ability["condition"] = conditions.remove(0),
        _ => ability["condition"] = json!({ "All": conditions }),
    }
    if p.get("SorcerySpeed") == Some("True") {
        ability["timing"] = json!("Sorcery");
    }
    if let Some(by) = by {
        if costs_the_source(&ability["cost"]) {
            return unmapped("key:AB.Activator-with-a-cost-of-the-source");
        }
        ability["by"] = json!(by);
    }
    match p.get("ActivationLimit") {
        None if words.once_per_turn => ability["limit"] = json!("OncePerTurn"),
        None => {}
        Some("1") => ability["limit"] = json!("OncePerTurn"),
        Some(times @ ("2" | "3")) => {
            ability["limit"] = json!({"TimesPerTurn": times.parse::<u32>().unwrap_or(2)})
        }
        Some(other) => return unmapped(format!("key:AB.ActivationLimit={other}")),
    }
    // Once a game is stricter than once a turn, so it replaces it.
    if game_limited {
        ability["limit"] = json!("OncePerGame");
    }
    match (p.get("ActivationZone"), words.from) {
        (None | Some("Battlefield"), None) => {}
        (None, Some(zone)) | (Some(zone @ ("Graveyard" | "Hand" | "Exile")), None) => {
            ability["from"] = json!(zone);
        }
        (Some(zone), Some(from)) if zone == from => ability["from"] = json!(from),
        (Some(other), _) => return unmapped(format!("key:AB.ActivationZone={other}")),
    }
    if !targets.is_empty() {
        ability["targets"] = Value::Array(targets);
    }
    Ok(json!({ "Activated": ability }))
}

/// `Activator$`: "Any player may activate this ability" (`Player`) or "Only your opponents may activate" (`Opponent`); `None`
/// for the controller.
fn activator_word(p: &Params) -> Tr<Option<&'static str>> {
    Ok(match p.get("Activator") {
        None | Some("You") => None,
        Some("Player") => Some("Anyone"),
        Some("Opponent" | "Player.Opponent") => Some("Opponents"),
        Some(other) => return unmapped(format!("key:AB.Activator={other}")),
    })
}

/// Whether the cost taps or gives up the source: a player who does not control the permanent cannot pay it.
fn costs_the_source(cost: &Value) -> bool {
    let own = |item: &Value| {
        matches!(
            item.as_str(),
            Some("Tap" | "Untap" | "SacrificeSelf" | "ExileSelf" | "DiscardSelf")
        )
    };
    cost.as_array().is_some_and(|items| items.iter().any(own)) || own(cost)
}

/// `ReduceCost$ <count>`: "This ability costs {1} less to activate for each ..." (CR 602.2b), the `Changed` cost item that
/// takes one generic mana off per counted object. A fixed number is not a count and stays a gap.
fn reduce_cost(ctx: &Ctx, p: &Params) -> Tr<Option<Value>> {
    let Some(text) = p.get("ReduceCost") else {
        return Ok(None);
    };
    if text.trim().parse::<i32>().is_ok() {
        return unmapped("key:AB.ReduceCost=fixed");
    }
    let per = super::amount::amount(ctx, text)?;
    Ok(Some(
        json!({"Changed": {"change": "Reduce", "mana": "{1}", "per": per}}),
    ))
}

/// Whether the line is used once each game: `GameActivationLimit$ 1`, or `Exhaust$ True` ("activate each exhaust ability
/// only once", CR 702.177). Another limit stays a gap.
fn once_a_game(p: &Params) -> Tr<bool> {
    let mut limited = false;
    match p.get("GameActivationLimit") {
        None => {}
        Some("1") => limited = true,
        Some(other) => return unmapped(format!("key:AB.GameActivationLimit={other}")),
    }
    match p.get("Exhaust") {
        None => {}
        Some("True") => limited = true,
        Some(other) => return unmapped(format!("key:AB.Exhaust={other}")),
    }
    Ok(limited)
}

fn loyalty_ability(p: &Params, text: &str, effects: &[Value], targets: &[Value]) -> Tr<Value> {
    let Some(cost) = loyalty(text) else {
        return unmapped("cost:loyalty-shape");
    };
    if let Some(key) = [
        "SorcerySpeed",
        "PlayerTurn",
        "ActivationLimit",
        "ActivationZone",
    ]
    .iter()
    .find(|key| p.has(key))
    {
        return unmapped(format!("key:Loyalty.{key}"));
    }
    let mut ability = json!({"cost": cost, "effects": effects});
    if !targets.is_empty() {
        ability["targets"] = Value::Array(targets.to_vec());
    }
    Ok(json!({ "Loyalty": ability }))
}
