//! Effects: the chain of an ability line and its sub-abilities, and the verbs that act on players (damage, life, cards).

use serde_json::{Value, json};

use super::amount::amount;
use super::condition::prefixed;
use super::ctx::{Ctx, Tr, only, unmapped};
use super::remember;
use super::valid::{defined, subject};
use super::{effects_effect, effects_loop, effects_obj, effects_zone, tokens};
use crate::forge::script::Params;

/// The effects of an ability line and of the sub-abilities it names, in order.
pub fn chain(ctx: &mut Ctx, params: &Params) -> Tr<Vec<Value>> {
    chain_at(ctx, params, true)
}

/// The effects of a line and of the sub-abilities after it. A part that is not the ability itself (`top`) and has a cost
/// is paid as it resolves: its effects and those after it happen if the cost is paid.
pub fn chain_at(ctx: &mut Ctx, p: &Params, top: bool) -> Tr<Vec<Value>> {
    ctx.mark_parent();
    let (condition, own) = prefixed(ctx, p)?;
    let mut effects = line(ctx, &own.without(&["UnlessCost", "UnlessPayer"]))?;
    if let Some(unless) = p.get("UnlessCost") {
        effects = if p.get("UnlessSwitched") == Some("True") {
            pays_to_do(ctx, p, unless, &effects)?
        } else {
            unless_pays(ctx, p, unless, &effects)?
        };
    }
    if let Some(condition) = condition {
        effects = vec![json!({"If": {"condition": condition, "then": effects}})];
    }
    if let Some(name) = p.get("SubAbility") {
        let next = ctx.sub(name)?;
        effects.extend(chain_at(ctx, &next, false)?);
    }
    match p.get("Cost") {
        Some(text) if !top => {
            let cost = super::cost::cost(ctx, text)?;
            Ok(vec![json!({"Pay": {"cost": cost, "paid": effects}})])
        }
        _ => Ok(effects),
    }
}

/// `UnlessCost$`: the effects happen unless the player named by `UnlessPayer$` pays.
fn unless_pays(ctx: &Ctx, p: &Params, unless: &str, effects: &[Value]) -> Tr<Vec<Value>> {
    let who = match p.get("UnlessPayer") {
        Some("You") => json!("You"),
        Some("TargetedController") => json!({ "ControllerOf": ctx.last_target()? }),
        None if api_of(p)? == "Counter" => json!({ "ControllerOf": ctx.last_target()? }),
        Some(other) => return unmapped(format!("key:Unless.UnlessPayer={other}")),
        None => return unmapped("key:Unless.no-UnlessPayer"),
    };
    let Value::String(cost) = super::cost::cost(ctx, unless)? else {
        return unmapped("key:Unless.UnlessCost-shape");
    };
    if cost.contains('X') {
        return unmapped("key:Unless.UnlessCost-X");
    }
    Ok(vec![
        json!({"Pay": {"who": who, "cost": cost, "unpaid": effects}}),
    ])
}

/// `UnlessSwitched$ True`: the effects happen if the player pays the cost ("you may pay; if you do").
fn pays_to_do(ctx: &Ctx, p: &Params, cost: &str, effects: &[Value]) -> Tr<Vec<Value>> {
    if p.get("UnlessPayer") != Some("You") {
        return unmapped("key:Unless.UnlessSwitched-payer");
    }
    let Value::String(cost) = super::cost::cost(ctx, cost)? else {
        return unmapped("key:Unless.UnlessCost-shape");
    };
    if cost.contains('X') {
        return unmapped("key:Unless.UnlessCost-X");
    }
    Ok(vec![json!({"Pay": {"cost": cost, "paid": effects}})])
}

/// The API a line names.
fn api_of(params: &Params) -> Tr<&str> {
    ["SP", "AB", "DB"]
        .into_iter()
        .find_map(|head| params.get(head))
        .map_or_else(|| unmapped("line:no-api"), Ok)
}

/// The effects of one line, without its sub-abilities.
/// One line: its effects, and the latest target it declared as what `RememberTargets` leaves to remember when the API does not
/// move it (a move names what it moved).
fn line(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let api = api_of(p)?;
    if p.get("RememberTargets") != Some("True") || matches!(api, "ChangeZone" | "Destroy") {
        return api_line(ctx, p, api);
    }
    if !p.has("ValidTgts") {
        return unmapped("key:RememberTargets-without-targets");
    }
    let effects = api_line(ctx, &p.without(&["RememberTargets"]), api)?;
    ctx.remembered = Some(ctx.last_target()?);
    Ok(effects)
}

fn api_line(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    let one = |value: Tr<Value>| value.map(|value| vec![value]);
    if let Some(found) = super::effects_game::effects(ctx, p, api) {
        return found;
    }
    if let Some(found) = super::effects_copy::effects(ctx, p, api) {
        return found;
    }
    if let Some(found) = super::effects_verbs::effects(ctx, p, api) {
        return found;
    }
    if let Some(found) = super::effects_verbs2::effects(ctx, p, api) {
        return found;
    }
    match api {
        "DealDamage" => one(deal_damage(ctx, p)),
        "Draw" => one(draw(ctx, p)),
        "GainLife" => one(life(ctx, p, "GainLife")),
        "LoseLife" => one(life(ctx, p, "LoseLife")),
        "Discard" => one(discard(ctx, p)),
        "Mill" => one(mill(ctx, p)),
        "Scry" => one(count_verb(ctx, p, "Scry", "ScryNum")),
        "Surveil" => one(count_verb(ctx, p, "Surveil", "Amount")),
        "Proliferate" => one(proliferate(p)),
        "Cleanup" => cleanup(p),
        "Effect" => effects_effect::effects(ctx, p),
        "RepeatEach" => effects_loop::repeat_each(ctx, p),
        "ImmediateTrigger" => effects_loop::immediate_trigger(ctx, p),
        "PeekAndReveal" => effects_loop::peek_and_reveal(ctx, p),
        "RollDice" => effects_loop::roll_dice(ctx, p),
        "ChooseCard" => effects_loop::choose_card(ctx, p),
        "ChooseColor" | "ChooseType" => effects_loop::choose_value(ctx, p, api),
        "Charm" => one(charm(ctx, p)),
        "DelayedTrigger" => one(delayed(ctx, p)),
        "Dig" => effects_zone::dig(ctx, p),
        "DigUntil" => effects_zone::dig_until(ctx, p),
        "Attach" => effects_obj::attach(ctx, p),
        "CopyPermanent" => tokens::copy_permanent(ctx, p),
        "Destroy" | "DestroyAll" | "Counter" | "Pump" | "PumpAll" | "PutCounter"
        | "RemoveCounter" | "Tap" | "Untap" | "TapAll" | "UntapAll" | "Sacrifice"
        | "Regenerate" | "Fight" | "GainControl" | "DamageAll" | "Animate" | "PreventDamage"
        | "PutCounterAll" | "AnimateAll" => effects_obj::effects(ctx, p, api),
        "ChangeZone" | "ChangeZoneAll" => effects_zone::effects(ctx, p, api),
        "Token" | "Mana" | "Investigate" => tokens::effects(ctx, p, api),
        other => unmapped(format!("effect:{other}")),
    }
}

/// `Cleanup`: bookkeeping of remembered objects, which the translator does not use.
fn cleanup(p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Cleanup",
        &["ClearRemembered", "ClearImprinted", "ClearTriggered"],
    )?;
    Ok(Vec::new())
}

/// Whether the line divides its amount among targets (`DividedAsYouChoose$`); the total it names must be the amount.
pub fn divided(p: &Params, amount_key: &str) -> Tr<bool> {
    match p.get("DividedAsYouChoose") {
        None => Ok(false),
        Some(total) if p.get(amount_key).is_none_or(|amount| amount == total) => Ok(true),
        Some(_) => unmapped(format!("key:{amount_key}-differs-from-DividedAsYouChoose")),
    }
}

fn deal_damage(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "DealDamage",
        &[
            "NumDmg",
            "ValidTgts",
            "Defined",
            "TargetMin",
            "TargetMax",
            "DamageSource",
            "DividedAsYouChoose",
        ],
    )?;
    let to = subject(ctx, p, "Self", None)?;
    let amount = if divided(p, "NumDmg")? {
        json!("Divided")
    } else {
        amount(ctx, p.get("NumDmg").unwrap_or("1"))?
    };
    let mut effect = json!({"to": to, "amount": amount});
    if let Some(source) = p.get("DamageSource") {
        effect["source"] = defined(ctx, source)?;
    }
    Ok(json!({ "DealDamage": effect }))
}

/// The player of a line: its target or `Defined$` word, `You` when it has neither.
fn player(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    subject(ctx, p, "You", None)
}

fn draw(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "Draw",
        &["NumCards", "Defined", "ValidTgts", "TargetMin", "TargetMax"],
    )?;
    let who = player(ctx, p)?;
    let count = amount(ctx, p.get("NumCards").unwrap_or("1"))?;
    Ok(json!({"Draw": {"who": who, "count": count}}))
}

fn life(ctx: &mut Ctx, p: &Params, verb: &str) -> Tr<Value> {
    only(
        p,
        verb,
        &[
            "LifeAmount",
            "Defined",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
        ],
    )?;
    let who = player(ctx, p)?;
    let amount = amount(ctx, p.get("LifeAmount").unwrap_or("1"))?;
    Ok(json!({ verb: {"who": who, "amount": amount} }))
}

fn discard(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "Discard",
        &[
            "NumCards",
            "Defined",
            "ValidTgts",
            "Mode",
            "TargetMin",
            "TargetMax",
            "RememberDiscarded",
        ],
    )?;
    let random = match p.get("Mode") {
        None | Some("TgtChoose") => false,
        Some("Random") => true,
        Some(other) => return unmapped(format!("key:Discard.Mode={other}")),
    };
    let who = player(ctx, p)?;
    let count = amount(ctx, p.get("NumCards").unwrap_or("1"))?;
    let mut discard = json!({"who": who, "count": count, "random": random});
    if p.get("RememberDiscarded") == Some("True") {
        let name = ctx.fresh("discarded");
        ctx.remembered = Some(json!({ "Bound": name }));
        discard["as"] = json!(name);
    }
    Ok(json!({ "Discard": discard }))
}

/// `Mill`, naming the cards milled for `RememberMilled`.
fn mill(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let milled = remember::wanted(p, &["RememberMilled"]);
    let mut effect = count_verb(ctx, &p.without(&["RememberMilled"]), "Mill", "NumCards")?;
    if milled {
        remember::bind_moved(ctx, &mut effect, "Mill", "milled");
    }
    Ok(effect)
}

/// A verb of a player and a count: mill, scry, surveil.
fn count_verb(ctx: &mut Ctx, p: &Params, verb: &str, key: &str) -> Tr<Value> {
    only(
        p,
        verb,
        &[key, "Defined", "ValidTgts", "TargetMin", "TargetMax"],
    )?;
    let who = player(ctx, p)?;
    let count = amount(ctx, p.get(key).unwrap_or("1"))?;
    Ok(json!({ verb: {"who": who, "count": count} }))
}

/// `Charm`: a modal spell or ability whose modes are the named parts of `Choices$`, each with targets of its own.
fn charm(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "Charm", &["Choices", "CharmNum", "MinCharmNum"])?;
    let Some(choices) = p.get("Choices") else {
        return unmapped("key:Charm.no-Choices");
    };
    let outer = std::mem::take(&mut ctx.targets);
    let mut modes = Vec::new();
    for name in choices.split(',') {
        let part = ctx.sub(name)?;
        let effects = chain_at(ctx, &part, true)?;
        let mut mode = json!({ "effects": effects });
        if !ctx.targets.is_empty() {
            mode["targets"] = Value::Array(std::mem::take(&mut ctx.targets));
        }
        modes.push(mode);
    }
    ctx.targets = outer;
    let max = p.get("CharmNum").unwrap_or("1");
    let min = p.get("MinCharmNum").unwrap_or(max);
    let (min, max) = (amount(ctx, min)?, amount(ctx, max)?);
    let mut modal = json!({ "modes": modes });
    if (&min, &max) != (&json!(1), &json!(1)) {
        modal["choose"] = json!({"min": min, "max": max});
    }
    Ok(json!({ "Modal": modal }))
}

/// `DelayedTrigger` at a step ("at the beginning of the next end step, ..."): the part named by `Execute$` happens then.
fn delayed(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "DelayedTrigger",
        &[
            "Mode",
            "Phase",
            "ValidPlayer",
            "Execute",
            "NextTurn",
            "RememberObjects",
        ],
    )?;
    if p.get("Mode") != Some("Phase") {
        return unmapped("key:DelayedTrigger.Mode");
    }
    // "Next turn" skips a step still to come this turn; a delayed trigger here fires at the next such step.
    if p.get("NextTurn").is_some() {
        return unmapped("key:DelayedTrigger.NextTurn");
    }
    let step = super::triggers::step(p.get("Phase").unwrap_or_default())?;
    let whose = match p.get("ValidPlayer") {
        None | Some("Player") => "EachPlayer",
        Some("You") => "You",
        Some(other) => return unmapped(format!("key:DelayedTrigger.ValidPlayer={other}")),
    };
    let Some(execute) = p.get("Execute") else {
        return unmapped("key:DelayedTrigger.no-Execute");
    };
    match p.get("RememberObjects") {
        None => {}
        Some("Remembered" | "RememberedLKI") if ctx.remembered.is_some() => {}
        Some(other) => return unmapped(format!("key:DelayedTrigger.RememberObjects={other}")),
    }
    let part = ctx.sub(execute)?;
    let effects = chain_at(ctx, &part, false)?;
    let when = json!({"StepBegins": {"step": step, "whose": whose}});
    Ok(json!({"DelayedTrigger": {"when": when, "effects": effects}}))
}

fn proliferate(p: &Params) -> Tr<Value> {
    only(p, "Proliferate", &[])?;
    Ok(json!({"Proliferate": {}}))
}
