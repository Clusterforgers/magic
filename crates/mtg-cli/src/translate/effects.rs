//! Effects: the chain of an ability line and its sub-abilities, and the verbs that act on players (damage, life, cards).

use serde_json::{Value, json};

use super::amount::amount;
use super::condition::prefixed;
use super::ctx::{Ctx, Tr, only, unmapped};
use super::remember;
use super::valid::{defined, objects, selector_any, subject};
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
    // A condition on a line that picks its own target reads that target, which the line declares only after it.
    let coming = p
        .has("ValidTgts")
        .then(|| u8::try_from(ctx.targets.len()).ok())
        .flatten();
    ctx.coming = coming;
    let stated = prefixed(ctx, p);
    ctx.coming = None;
    let (condition, own) = stated?;
    let unless_keys = [
        "UnlessCost",
        "UnlessPayer",
        "UnlessSwitched",
        "UnlessResolveSubs",
    ];
    let asks = asks_may(p)?;
    let spent: &[&str] = match (asks, p.has("OptionalDecider")) {
        (false, _) => &[],
        (true, true) => &["OptionalDecider"],
        (true, false) => &["Optional"],
    };
    let mut effects = line(ctx, &own.without(&unless_keys).without(spent))?;
    if let (Some(index), Some(condition)) = (coming, &condition)
        && usize::from(index) >= ctx.targets.len()
        && condition
            .to_string()
            .contains(&json!({ "Target": index }).to_string())
    {
        return unmapped("defined:Targeted-without-target");
    }
    if asks {
        let who = may_decider(ctx, p)?;
        let mut node = serde_json::Map::new();
        if who != json!("You") {
            node.insert("who".into(), who);
        }
        node.insert("effects".into(), json!(effects));
        effects = vec![json!({ "May": node })];
    }
    let mut rest = match p.get("SubAbility") {
        Some(name) => {
            let next = ctx.sub(name)?;
            chain_at(ctx, &next, false)?
        }
        None => Vec::new(),
    };
    if let Some(unless) = p.get("UnlessCost") {
        if condition.is_some() && p.has("UnlessResolveSubs") {
            return unmapped("key:Unless.UnlessResolveSubs-with-condition");
        }
        effects = unless_pays(ctx, p, unless, effects, &mut rest)?;
    } else if unless_keys.iter().any(|key| p.has(key)) {
        return unmapped("key:Unless.without-UnlessCost");
    }
    if let Some(condition) = condition {
        effects = vec![json!({"If": {"condition": condition, "then": effects}})];
    }
    effects.extend(rest);
    match p.get("Cost") {
        Some(text) if !top => {
            let cost = super::cost::cost(ctx, text)?;
            Ok(vec![json!({"Pay": {"cost": cost, "paid": effects}})])
        }
        _ => Ok(effects),
    }
}

/// APIs whose `Optional$ True` asks the player who gets the effect whether to do all of it ("you may draw a card"). The
/// other APIs read the key themselves, with a meaning of their own (a count, a choice), or leave it a named gap.
const MAY_APIS: [&str; 9] = [
    "Draw",
    "Discard",
    "Mill",
    "GainLife",
    "LoseLife",
    "Untap",
    "Tap",
    "PutCounter",
    "Animate",
];

/// Whether the line asks a player to do it: `OptionalDecider$`, or `Optional$ True` on an API of `MAY_APIS`.
fn asks_may(p: &Params) -> Tr<bool> {
    let optional = p.get("Optional") == Some("True");
    Ok(p.has("OptionalDecider") || (optional && MAY_APIS.contains(&api_of(p)?)))
}

/// The player who decides, once the line has declared its targets: the one `OptionalDecider$` names, or you for `Optional$`
/// on an effect that acts on you. A line with sub-abilities stays a gap: whether they follow a refusal is not known here.
fn may_decider(ctx: &Ctx, p: &Params) -> Tr<Value> {
    let decider = p.get("OptionalDecider");
    if p.has("SubAbility") {
        return unmapped("key:May.with-SubAbility");
    }
    let who = match decider {
        Some(word) => match defined(ctx, word)? {
            group if matches!(group.as_str(), Some("Opponents" | "EachPlayer")) => {
                return unmapped(format!("key:May.OptionalDecider={word}"));
            }
            single => single,
        },
        None if !p.has("ValidTgts") && matches!(p.get("Defined"), None | Some("You")) => {
            json!("You")
        }
        None => return unmapped("key:May.Optional-decider"),
    };
    Ok(who)
}

/// `UnlessCost$`: the player named by `UnlessPayer$` may pay; the effects happen if they do not (`UnlessSwitched$ True`: if
/// they do). `UnlessResolveSubs$` says in which case the sub-abilities (`rest`) happen too, moved into that branch; without
/// it they happen either way.
fn unless_pays(
    ctx: &Ctx,
    p: &Params,
    unless: &str,
    effects: Vec<Value>,
    rest: &mut Vec<Value>,
) -> Tr<Vec<Value>> {
    let switched = p.get("UnlessSwitched") == Some("True");
    let subs_when_paid = match p.get("UnlessResolveSubs") {
        None => None,
        Some("WhenPaid") => Some(true),
        Some("WhenNotPaid") => Some(false),
        Some(other) => return unmapped(format!("key:Unless.UnlessResolveSubs={other}")),
    };
    let who = match p.get("UnlessPayer") {
        None if api_of(p)? == "Counter" => json!({ "ControllerOf": ctx.last_target()? }),
        None => return unmapped("key:Unless.no-UnlessPayer"),
        Some(word) => match defined(ctx, word)? {
            // Nobody decides for a group: whether any of them pays is not one player's choice.
            group if matches!(group.as_str(), Some("Opponents" | "EachPlayer")) => {
                return unmapped(format!("key:Unless.UnlessPayer={word}"));
            }
            single => single,
        },
    };
    let cost = unless_cost(ctx, unless)?;
    let (mut paid, mut unpaid) = (Vec::new(), Vec::new());
    if switched {
        paid.extend(effects);
    } else {
        unpaid.extend(effects);
    }
    match subs_when_paid {
        Some(true) => paid.append(rest),
        Some(false) => unpaid.append(rest),
        None => {}
    }
    let mut pay = serde_json::Map::new();
    if !(switched && who == json!("You")) {
        pay.insert("who".into(), who);
    }
    pay.insert("cost".into(), cost);
    if !paid.is_empty() {
        pay.insert("paid".into(), json!(paid));
    }
    if !unpaid.is_empty() {
        pay.insert("unpaid".into(), json!(unpaid));
    }
    Ok(vec![json!({ "Pay": pay })])
}

/// The cost an `UnlessCost$` names: mana and the items `Pay` asks the payer for (life, discard, sacrifice, energy);
/// a cost that reads an announced X stays a gap because nobody announces it here.
fn unless_cost(ctx: &Ctx, text: &str) -> Tr<Value> {
    let cost = super::cost::cost(ctx, text)?;
    if reads_x(&cost) {
        return unmapped("key:Unless.UnlessCost-X");
    }
    Ok(cost)
}

/// Whether a serialized cost holds an `{X}` of mana or the announced X as an amount.
fn reads_x(value: &Value) -> bool {
    match value {
        Value::String(text) => text == "X" || text.contains("X}"),
        Value::Array(items) => items.iter().any(reads_x),
        Value::Object(fields) => fields
            .iter()
            .any(|(key, inner)| key == "X" || reads_x(inner)),
        _ => false,
    }
}

/// The API a line names.
fn api_of(params: &Params) -> Tr<&str> {
    ["SP", "AB", "DB"]
        .into_iter()
        .find_map(|head| params.get(head))
        .map_or_else(|| unmapped("line:no-api"), Ok)
}

/// One line: its effects, with the objects it remembers or imprints bound (`memory::line`).
fn line(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let api = api_of(p)?;
    super::memory::line(ctx, p, api, api_line)
}

fn api_line(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    let one = |value: Tr<Value>| value.map(|value| vec![value]);
    if let Some(found) = super::effects_game::effects(ctx, p, api) {
        return found;
    }
    if let Some(found) = super::effects_copy::effects(ctx, p, api) {
        return found;
    }
    if let Some(found) = super::effects_digital::effects(ctx, p, api) {
        return found;
    }
    if let Some(found) = super::effects_verbs::effects(ctx, p, api) {
        return found;
    }
    if let Some(found) = super::effects_verbs2::effects(ctx, p, api) {
        return found;
    }
    if let Some(found) = super::effects_verbs3::effects(ctx, p, api) {
        return found;
    }
    if let Some(found) = super::effects_verbs4::effects(ctx, p, api) {
        return found;
    }
    if let Some(found) = super::effects_verbs5::effects(ctx, p, api) {
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
        "ChooseNumber" => effects_loop::choose_number(ctx, p),
        "AddPhase" => one(effects_loop::add_phase(p)),
        "TwoPiles" => effects_loop::two_piles(ctx, p),
        "Charm" => one(charm(ctx, p)),
        "DelayedTrigger" => one(delayed(ctx, p)),
        "Dig" => effects_zone::dig(ctx, p),
        "DigUntil" => effects_zone::dig_until(ctx, p),
        "ManifestDread" => effects_zone::manifest_dread(ctx, p),
        "Attach" => effects_obj::attach(ctx, p),
        "CopyPermanent" => super::tokens_copy::copy_permanent(ctx, p),
        "Destroy" | "DestroyAll" | "Counter" | "Pump" | "PumpAll" | "PutCounter"
        | "RemoveCounter" | "Tap" | "Untap" | "TapAll" | "UntapAll" | "Sacrifice"
        | "Regenerate" | "Fight" | "GainControl" | "DamageAll" | "Animate" | "PreventDamage"
        | "DamageResolve" | "PutCounterAll" | "AnimateAll" => effects_obj::effects(ctx, p, api),
        "ChangeZone" | "ChangeZoneAll" => effects_zone::effects(ctx, p, api),
        "Token" | "Mana" | "ManaReflected" | "Investigate" => tokens::effects(ctx, p, api),
        other => unmapped(format!("effect:{other}")),
    }
}

/// `Cleanup`: bookkeeping of remembered objects, which the translator does not use.
fn cleanup(p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Cleanup",
        &[
            "ClearRemembered",
            "ClearImprinted",
            "ClearTriggered",
            "ClearChosenCard",
        ],
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
            "DamageMap",
            "NumDmg",
            "ValidTgts",
            "Defined",
            "TargetMin",
            "TargetMax",
            "DamageSource",
            "DividedAsYouChoose",
        ],
    )?;
    super::effects_obj::damage_map(p)?;
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
            "DiscardValid",
            "DiscardValidDesc",
        ],
    )?;
    let whole_hand = p.get("Mode") == Some("Hand");
    let random = match p.get("Mode") {
        None | Some("TgtChoose" | "Hand") => false,
        Some("Random") => true,
        Some(other) => return unmapped(format!("key:Discard.Mode={other}")),
    };
    if whole_hand && (p.has("DiscardValid") || p.has("NumCards")) {
        return unmapped("key:Discard.Mode=Hand+count");
    }
    let filter = match p.get("DiscardValid") {
        None | Some("Card") => None,
        // A random pick among the cards that match is not a discard the engine makes.
        Some(_) if random => return unmapped("key:Discard.DiscardValid+Random"),
        Some(spec) => Some(selector_any(ctx, spec)?),
    };
    let who = player(ctx, p)?;
    let count = if whole_hand {
        // One player's hand as it is when the discard happens; a group would count every hand together.
        if matches!(who.as_str(), Some("Opponents" | "EachPlayer")) {
            return unmapped("key:Discard.Mode=Hand+group");
        }
        json!({"Count": [{"In": "Hand"}, {"OwnedBy": who}]})
    } else {
        amount(ctx, p.get("NumCards").unwrap_or("1"))?
    };
    let mut discard = json!({"who": who, "count": count, "random": random});
    if let Some(filter) = filter {
        discard["filter"] = json!(filter);
    }
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
    let Some(execute) = p.get("Execute") else {
        return unmapped("key:DelayedTrigger.no-Execute");
    };
    // The objects the trigger is about, which `Card.IsTriggerRemembered` in its event names.
    let pin = match p.get("RememberObjects") {
        None => None,
        Some("Remembered" | "RememberedLKI") if ctx.remembered.is_some() => ctx.remembered.clone(),
        Some("Targeted") => Some(objects(ctx.last_target()?)?),
        Some("ParentTarget") => Some(objects(ctx.parent()?)?),
        Some("Self") => Some(json!("This")),
        Some(other) => return unmapped(format!("key:DelayedTrigger.RememberObjects={other}")),
    };
    let (when, this_turn) = if p.get("Mode") == Some("Phase") {
        // A step has no event to name the objects in; the lines after it read them as remembered.
        if let Some(other) = p
            .get("RememberObjects")
            .filter(|word| !matches!(*word, "Remembered" | "RememberedLKI"))
        {
            return unmapped(format!("key:DelayedTrigger.RememberObjects={other}"));
        }
        (delayed_step(p)?, false)
    } else {
        // The ability's own keys are not part of the event.
        let event = p.without(&[
            "SP",
            "AB",
            "DB",
            "Cost",
            "SubAbility",
            "SorcerySpeed",
            "PlayerTurn",
        ]);
        let before = std::mem::replace(&mut ctx.pinned, pin);
        let read = super::triggers::delayed_when(ctx, &event);
        ctx.pinned = before;
        read?
    };
    // The `Triggered*` words of what it does are about the event it waited for.
    let shape = (p.get("Mode") != Some("Phase")).then(|| super::triggered::Shape {
        mode: p.get("Mode").unwrap_or_default().to_owned(),
        itself: super::triggers::is_about_itself(&when),
        by: when
            .get("BecomesBlocked")
            .is_some_and(|node| node.get("by").is_some()),
    });
    let outer = std::mem::replace(&mut ctx.trigger, shape);
    let effects = ctx
        .sub(execute)
        .and_then(|part| chain_at(ctx, &part, false));
    ctx.trigger = outer;
    let effects = effects?;
    let mut node = json!({"DelayedTrigger": {"when": when, "effects": effects}});
    if this_turn {
        node["DelayedTrigger"]["this_turn"] = json!(true);
    }
    Ok(node)
}

/// The pattern of a delayed trigger at a step: the step and whose it is.
fn delayed_step(p: &Params) -> Tr<Value> {
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
    // "Next turn" skips a step still to come this turn. Nobody holds priority before the upkeep has begun, so for an
    // upkeep of every player the next such step is always the next turn's; a later step could still be this turn's, and
    // "your" upkeep may not be the next turn's.
    let every_upkeep = matches!(p.get("ValidPlayer"), None | Some("Player"));
    if p.get("NextTurn").is_some() && !(p.get("Phase") == Some("Upkeep") && every_upkeep) {
        return unmapped("key:DelayedTrigger.NextTurn");
    }
    let step = super::triggers::step(p.get("Phase").unwrap_or_default())?;
    let whose = match p.get("ValidPlayer") {
        None | Some("Player") => "EachPlayer",
        Some("You") => "You",
        Some(other) => return unmapped(format!("key:DelayedTrigger.ValidPlayer={other}")),
    };
    Ok(json!({"StepBegins": {"step": step, "whose": whose}}))
}

fn proliferate(p: &Params) -> Tr<Value> {
    only(p, "Proliferate", &[])?;
    Ok(json!({"Proliferate": {}}))
}
