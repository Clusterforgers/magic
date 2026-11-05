//! Effects: the chain of an ability line and its sub-abilities, and the verbs that act on players (damage, life, cards).

use serde_json::{Value, json};

use super::amount::amount;
use super::condition::prefixed;
use super::ctx::{Ctx, Tr, flag, only, unmapped};
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
    let before = (ctx.remembered.clone(), ctx.imprinted.clone());
    let excess = ctx.excess.clone();
    let mut effects = line(ctx, &own.without(&unless_keys).without(spent))?;
    // A name bound inside a condition or a "may" stays for the lines after it that count it or test that it holds something.
    if asks || condition.is_some() {
        ctx.excess = excess;
        for (now, was) in [(&ctx.remembered, &before.0), (&ctx.imprinted, &before.1)] {
            if now != was
                && let Some(name) = now
                    .as_ref()
                    .and_then(|held| held.get("Bound"))
                    .and_then(Value::as_str)
            {
                ctx.scoped.insert(name.to_owned());
            }
        }
    }
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
        effects = super::frame::ask(effects, &who);
    }
    let mut rest = match p.get("SubAbility") {
        Some(name) => {
            let next = ctx.sub(name)?;
            chain_at(ctx, &next, false)?
        }
        None => Vec::new(),
    };
    // A roll read only as a number: the lines after it run in its one row, where the result is known.
    if p.has("ResultSVar")
        && !rest.is_empty()
        && let Some(roll) = effects.last_mut().and_then(|e| e.get_mut("RollDice"))
        && roll["results"].as_array().is_some_and(Vec::is_empty)
    {
        roll["results"] = json!([{"from": 1, "effects": std::mem::take(&mut rest)}]);
    }
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
            if let Some((verb, taken)) = taken_read_after(&cost, &effects) {
                return Ok(taken_then(ctx, verb, &taken, effects));
            }
            Ok(vec![json!({"Pay": {"cost": cost, "paid": effects}})])
        }
        _ => Ok(effects),
    }
}

/// The sacrifice or discard a payment is when it is the whole cost and the effects after it read what it took ("the
/// sacrificed creature's power"): a payment names no objects to its effects, a step does.
fn taken_read_after(cost: &Value, effects: &[Value]) -> Option<(&'static str, Value)> {
    let [item] = cost.as_array()?.as_slice() else {
        return None;
    };
    let (verb, taken) = ["Sacrifice", "Discard"]
        .into_iter()
        .find_map(|verb| item.get(verb).map(|taken| (verb, taken.clone())))?;
    let reads = Value::Array(effects.to_vec())
        .to_string()
        .contains("\"Paid\"");
    reads.then_some((verb, taken))
}

/// "You may sacrifice or discard ...; if you do, ...": the step names what it took and the effects after it read that name.
fn taken_then(ctx: &mut Ctx, verb: &str, taken: &Value, mut effects: Vec<Value>) -> Vec<Value> {
    let name = ctx.fresh(if verb == "Sacrifice" {
        "sacrificed"
    } else {
        "discarded"
    });
    let mut step = json!({ verb: taken.clone() });
    step[verb]["as"] = json!(name);
    for effect in &mut effects {
        name_paid(effect, &name);
    }
    // A sacrificed token is gone, so the number moved says it; a discarded card is in the graveyard to count.
    let did = if verb == "Sacrifice" {
        json!({"Compare": {"left": {"Bound": format!("{name}~moved")}, "op": "Ge", "right": 1}})
    } else {
        json!({"CountOf": {"op": "Ge", "right": 1, "selector": [], "subject": {"Bound": name}}})
    };
    super::frame::ask(
        vec![step, json!({"If": {"condition": did, "then": effects}})],
        &json!("You"),
    )
}

/// Replaces every read of `Paid` with the name of the step that took the objects.
fn name_paid(value: &mut Value, name: &str) {
    match value {
        Value::String(word) if word == "Paid" => *value = json!({"Bound": name}),
        Value::Array(items) => items.iter_mut().for_each(|item| name_paid(item, name)),
        Value::Object(map) => map.values_mut().for_each(|item| name_paid(item, name)),
        _ => {}
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

/// A discard whose `Optional$` is the count itself ("discard up to two cards"): the chooser of a revealed hand, or a count
/// above one that no `OptionalDecider$` hands to someone else. A single card is "you may discard", the usual May.
fn own_optional(p: &Params) -> bool {
    match p.get("Mode") {
        Some("RevealYouChoose" | "LookYouChoose") => true,
        Some("Hand" | "Random") => false,
        _ => {
            !p.has("OptionalDecider")
                && (p.get("AnyNumber") == Some("True")
                    || p.get("NumCards").is_some_and(|n| n != "1"))
        }
    }
}

/// Whether the line asks a player to do it: `OptionalDecider$`, or `Optional$ True` on an API of `MAY_APIS`.
fn asks_may(p: &Params) -> Tr<bool> {
    let optional = p.get("Optional") == Some("True");
    // On a discard the caster chooses, "optional" is the choice of the card, which the discard itself carries.
    let own = api_of(p)? == "Discard" && own_optional(p);
    let optional = optional && !own;
    Ok(p.has("OptionalDecider") || (optional && MAY_APIS.contains(&api_of(p)?)))
}

/// The player who decides, once the line has declared its targets: the one `OptionalDecider$` names, or you for `Optional$`
/// on an effect that acts on you. The sub-abilities after it are not part of the choice: they happen either way, as the Oracle text "You may X. Then Y" says.
fn may_decider(ctx: &Ctx, p: &Params) -> Tr<Value> {
    let decider = p.get("OptionalDecider");
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
        // The objects these verbs act on make no choice; whoever controls the ability says whether it is done.
        None if matches!(api_of(p)?, "PutCounter" | "Animate" | "Tap" | "Untap") => json!("You"),
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
        // A group ("unless any player pays") is asked in turn order until one pays; every seat gets the normal clock and an
        // unanswered question is a no.
        Some(word) => defined(ctx, word)?,
    };
    let (cost, times) = unless_cost(ctx, unless)?;
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
    if let Some(times) = times {
        pay.insert("times".into(), times);
    }
    if !paid.is_empty() {
        pay.insert("paid".into(), json!(paid));
    }
    if !unpaid.is_empty() {
        pay.insert("unpaid".into(), json!(unpaid));
    }
    Ok(vec![json!({ "Pay": pay })])
}

/// The cost an `UnlessCost$` names: mana and the items `Pay` asks the payer for (life, discard, sacrifice, energy);
/// a cost of an amount (`X`, `PayLife<X>`, `PayEnergy<X>`) is one unit paid that many times at once, and the payer announces
/// nothing; any other cost that reads X stays a gap.
fn unless_cost(ctx: &Ctx, text: &str) -> Tr<(Value, Option<Value>)> {
    let word = text.trim();
    let unit = match word {
        "X" | "Y" => Some(("1".to_owned(), word)),
        _ => ["PayLife", "PayEnergy"].iter().find_map(|verb| {
            let inner = word
                .strip_prefix(verb)?
                .strip_prefix('<')?
                .strip_suffix('>')?;
            matches!(inner, "X" | "Y").then(|| (format!("{verb}<1>"), inner))
        }),
    };
    if let Some((unit, var)) = unit {
        let times = amount(ctx, var)?;
        return Ok((super::cost::cost(ctx, &unit)?, Some(times)));
    }
    let cost = super::cost::cost(ctx, text)?;
    if reads_x(&cost) {
        return unmapped("key:Unless.UnlessCost-X");
    }
    Ok((cost, None))
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
        "DealDamage" => {
            deal_damage(ctx, p).and_then(|node| super::effects_obj::with_dies_shield(ctx, p, node))
        }
        "Draw" => draw(ctx, p),
        "GainLife" => one(life(ctx, p, "GainLife")),
        "LoseLife" => one(life(ctx, p, "LoseLife")),
        "Discard" => discard(ctx, p),
        "Mill" => one(mill(ctx, p)),
        "Scry" => one(count_verb(ctx, p, "Scry", "ScryNum")),
        "Surveil" => one(count_verb(ctx, p, "Surveil", "Amount")),
        "Proliferate" => one(proliferate(p)),
        "Cleanup" => cleanup(p),
        "Effect" => effects_effect::effects(ctx, p),
        "RepeatEach" => effects_loop::repeat_each(ctx, p),
        "Repeat" => effects_loop::repeat(ctx, p),
        "ImmediateTrigger" => effects_loop::immediate_trigger(ctx, p),
        "PeekAndReveal" => effects_loop::peek_and_reveal(ctx, p),
        "RollDice" => effects_loop::roll_dice(ctx, p),
        "Vote" => super::effects_vote::vote(ctx, p),
        "ChooseCard" => effects_loop::choose_card(ctx, p),
        "ChooseColor" | "ChooseType" => effects_loop::choose_value(ctx, p, api),
        "ChooseNumber" => effects_loop::choose_number(ctx, p),
        "AddPhase" => one(effects_loop::add_phase(p)),
        "TwoPiles" => effects_loop::two_piles(ctx, p),
        "Charm" => one(charm(ctx, p)),
        "DelayedTrigger" => delayed(ctx, p),
        "Dig" => effects_zone::dig(ctx, p),
        "RearrangeTopOfLibrary" => effects_zone::rearrange_top(ctx, p),
        "DigUntil" => effects_zone::dig_until(ctx, p),
        "ManifestDread" => effects_zone::manifest_dread(ctx, p),
        "Attach" => effects_obj::attach(ctx, p),
        "CopyPermanent" => super::tokens_copy::copy_permanent(ctx, p),
        "Destroy" | "DestroyAll" | "Counter" | "Pump" | "PumpAll" | "PutCounter"
        | "RemoveCounter" | "Tap" | "Untap" | "TapAll" | "UntapAll" | "Sacrifice"
        | "Regenerate" | "Fight" | "GainControl" | "DamageAll" | "Animate" | "PreventDamage"
        | "DamageResolve" | "PutCounterAll" | "AnimateAll" | "Debuff" | "Protection"
        | "ProtectionAll" | "AlterAttribute" => effects_obj::effects(ctx, p, api),
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
            "ClearNamedCard",
            "ClearChosenPlayer",
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
            "ExcessSVar",
            "ExcessSVarCondition",
            "ExcessDamage",
            "ExcessDamageCondition",
            "ReplaceDyingDefined",
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
    let excess = excess_part(ctx, p)?;
    if let Some((part, name)) = excess {
        effect["excess"] = part;
        if let Some(name) = name {
            ctx.excess = Some(name);
        }
    }
    Ok(json!({ "DealDamage": effect }))
}

/// The node of an excess and the `SVar` and number name it sets, when it sets one.
type ExcessPart = (Value, Option<(String, String)>);

/// `ExcessSVar$` and `ExcessDamage$` of a damage line (CR 120.10): the node that keeps the damage beyond lethal as a number,
/// or deals it to someone else, and the `SVar` and number name the lines after it read. They need one targeted recipient:
/// a divided or several-target line, or a condition on which damaged object counts, is a gap.
fn excess_part(ctx: &mut Ctx, p: &Params) -> Tr<Option<ExcessPart>> {
    let (svar, instead) = (p.get("ExcessSVar"), p.get("ExcessDamage"));
    if svar.is_none() && instead.is_none() {
        return if p.has("ExcessSVarCondition") || p.has("ExcessDamageCondition") {
            unmapped("key:DealDamage.ExcessCondition-without-excess")
        } else {
            Ok(None)
        };
    }
    if svar.is_some() && instead.is_some() {
        return unmapped("key:DealDamage.ExcessSVar-with-ExcessDamage");
    }
    if !p.has("ValidTgts")
        || p.has("DividedAsYouChoose")
        || p.get("TargetMax").is_some_and(|max| max != "1")
    {
        return unmapped("key:DealDamage.Excess-without-one-target");
    }
    if p.has("ExcessDamageCondition") {
        return unmapped("key:DealDamage.ExcessDamageCondition");
    }
    if p.get("ExcessSVarCondition")
        .is_some_and(|c| c != "Card.targetedBy")
    {
        return unmapped("key:DealDamage.ExcessSVarCondition");
    }
    if let Some(who) = instead {
        if p.has("ExcessSVarCondition") {
            return unmapped("key:DealDamage.ExcessSVarCondition");
        }
        if !matches!(who, "TargetedController" | "ThisTargetedController") {
            return unmapped(format!("key:DealDamage.ExcessDamage={who}"));
        }
        return Ok(Some((
            json!({"to": defined(ctx, "TargetedController")?}),
            None,
        )));
    }
    let svar = svar.unwrap_or_default();
    let bound = ctx.fresh("excess");
    Ok(Some((json!({"as": bound}), Some((svar.to_owned(), bound)))))
}

/// The player of a line: its target or `Defined$` word, `You` when it has neither.
fn player(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    subject(ctx, p, "You", None)
}

/// `Draw`; with `Upto$ True` the drawer first picks how many, from none to `NumCards$` (CR 121.2), and draws that many.
fn draw(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Draw",
        &[
            "NumCards",
            "Defined",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
            "Upto",
            "RememberDrawn",
        ],
    )?;
    let who = player(ctx, p)?;
    let count = amount(ctx, p.get("NumCards").unwrap_or("1"))?;
    // `AllReplaced` also counts draws a replacement effect changed; the cards are those that were drawn.
    let remember = match p.get("RememberDrawn") {
        None | Some("False") => false,
        Some("True" | "AllReplaced") => true,
        Some(other) => return unmapped(format!("key:Draw.RememberDrawn={other}")),
    };
    let mut draw = json!({"Draw": {"who": who, "count": count}});
    if remember {
        if matches!(who.as_str(), Some("Opponents" | "EachPlayer")) {
            return unmapped("key:Draw.RememberDrawn.group");
        }
        remember::bind_moved(ctx, &mut draw, "Draw", "drawn");
    }
    match p.get("Upto") {
        None => Ok(vec![draw]),
        Some("True") => {
            if matches!(who.as_str(), Some("Opponents" | "EachPlayer")) {
                return unmapped("key:Draw.Upto.group");
            }
            let bind = ctx.fresh("number");
            draw["Draw"]["count"] = json!({"ChosenAs": bind});
            Ok(vec![
                json!({"ChooseValue": {"who": who, "kind": {"Number": [0, count]}, "as": bind}}),
                draw,
            ])
        }
        Some(other) => unmapped(format!("key:Draw.Upto={other}")),
    }
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

fn discard(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
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
            "AnyNumber",
            "Optional",
            "UnlessType",
        ],
    )?;
    let whole_hand = p.get("Mode") == Some("Hand");
    let look_only = p.get("Mode") == Some("LookYouChoose");
    let reveal_choose = look_only || p.get("Mode") == Some("RevealYouChoose");
    let reveal_all = p.get("Mode") == Some("RevealDiscardAll");
    if p.has("Optional") && !own_optional(p) {
        return unmapped("key:Discard.Optional");
    }
    let random = match p.get("Mode") {
        None
        | Some("TgtChoose" | "Hand" | "RevealYouChoose" | "LookYouChoose" | "RevealDiscardAll") => {
            false
        }
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
    let any_number = p.get("AnyNumber") == Some("True");
    if any_number
        && (p.get("Optional") != Some("True")
            || whole_hand
            || reveal_choose
            || random
            || p.has("NumCards"))
    {
        return unmapped("key:Discard.AnyNumber");
    }
    let count = if any_number || reveal_all {
        // "Any number" is up to the cards of the hand that may be chosen, "all" every one that matches; a group would
        // count every hand together, which only "all" can take.
        let group = matches!(who.as_str(), Some("Opponents" | "EachPlayer"));
        if any_number && group {
            return unmapped("key:Discard.AnyNumber+group");
        }
        if reveal_all && (filter.is_none() || p.has("NumCards") || p.has("Optional")) {
            return unmapped("key:Discard.Mode=RevealDiscardAll+shape");
        }
        let mut hand = vec![json!({"In": "Hand"}), json!({"OwnedBy": who})];
        hand.extend(filter.iter().flatten().cloned());
        json!({"Count": hand})
    } else if whole_hand {
        // A group counts every hand together, more than any one hand holds, so each hand is taken whole.
        json!({"Count": [{"In": "Hand"}, {"OwnedBy": who}]})
    } else {
        amount(ctx, p.get("NumCards").unwrap_or("1"))?
    };
    let mut discard = json!({"who": who, "count": count, "random": random});
    if p.get("Optional") == Some("True") {
        discard["optional"] = json!(true);
    }
    if reveal_choose {
        // "Reveals their hand, you choose": a hand that is the chooser's own has nothing to reveal.
        if who.as_str() == Some("You") {
            return unmapped("key:Discard.Mode=RevealYouChoose+You");
        }
        discard["by"] = json!("You");
    }
    if let Some(filter) = filter {
        discard["filter"] = json!(filter);
    }
    if p.get("RememberDiscarded") == Some("True") {
        let name = ctx.fresh("discarded");
        ctx.remembered = Some(json!({ "Bound": name }));
        discard["as"] = json!(name);
    }
    let discard = json!({ "Discard": discard });
    if let Some(unless) = p.get("UnlessType") {
        let plain = !(whole_hand || reveal_choose || random || any_number || p.has("Optional"));
        return unless_discard(ctx, p, unless, plain, &discard);
    }
    if look_only || !(reveal_choose || reveal_all) {
        return Ok(vec![discard]);
    }
    // "Look at" shows the hand to the chooser only, who sees it in the choice; "reveals" shows every seat.
    let text = ctx.oracle.to_lowercase();
    if text.contains("look at") && !text.contains("reveal") {
        return Ok(vec![discard]);
    }
    let hand = json!({"Reveal": {"what": {"Each": [{"OwnedBy": who}, {"In": "Hand"}]}}});
    Ok(vec![hand, discard])
}

/// "Discard two cards unless you discard a creature card": the player may instead pay one matching card. Only a plain
/// discard of a count has the form.
fn unless_discard(
    ctx: &mut Ctx,
    p: &Params,
    unless: &str,
    plain: bool,
    discard: &Value,
) -> Tr<Vec<Value>> {
    if !plain {
        return unmapped("key:Discard.UnlessType+shape");
    }
    let cost = json!([{"Discard": {"count": 1, "filter": selector_any(ctx, unless)?}}]);
    let who = player(ctx, p)?;
    Ok(vec![
        json!({"Pay": {"who": who, "cost": cost, "unpaid": [discard]}}),
    ])
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
    only(
        p,
        "Charm",
        &[
            "Choices",
            "CharmNum",
            "MinCharmNum",
            "ChoiceRestriction",
            "CanRepeatModes",
        ],
    )?;
    let keyword = ctx.modal_keyword.take();
    let extra = ctx.extra_mode_cost.take();
    let Some(choices) = p.get("Choices") else {
        return unmapped("key:Charm.no-Choices");
    };
    let outer = std::mem::take(&mut ctx.targets);
    let mut mode_costs = Vec::new();
    let mut modes = Vec::new();
    for name in choices.split(',') {
        let part = ctx.sub(name)?;
        mode_costs.push(
            part.get("ModeCost")
                .map(|cost| cost_of(ctx, cost))
                .transpose()?,
        );
        let effects = chain_at(ctx, &part.without(&["ModeCost"]), true)?;
        let mut mode = json!({ "effects": effects });
        if !ctx.targets.is_empty() {
            mode["targets"] = Value::Array(std::mem::take(&mut ctx.targets));
        }
        modes.push(mode);
    }
    ctx.targets = outer;
    // "Each mode must target a different player" is a rule across modes that the engine's targeting cannot enforce.
    if p.get("CharmNum") == Some("MaxUniqueOpponents") {
        return unmapped("key:Charm.DifferentPlayerPerMode");
    }
    let max = p.get("CharmNum").unwrap_or("1");
    let min = p.get("MinCharmNum").unwrap_or(max);
    let (min, max) = (amount(ctx, min)?, amount(ctx, max)?);
    let mut modal = json!({ "modes": modes });
    match (keyword, extra) {
        (Some("Spree" | "Tiered"), None) if mode_costs.iter().all(Option::is_some) => {
            modal["mode_costs"] = json!(mode_costs.into_iter().flatten().collect::<Vec<_>>());
        }
        (Some("Escalate"), Some(cost)) if mode_costs.iter().all(Option::is_none) => {
            modal["extra_mode_cost"] = cost_of(ctx, &cost)?;
        }
        (None, None) if mode_costs.iter().all(Option::is_none) => {}
        _ => return unmapped("key:Charm.ModeCost"),
    }
    if (&min, &max) != (&json!(1), &json!(1)) {
        modal["choose"] = json!({"min": min, "max": max});
    }
    // A mode chosen earlier in the turn or the game is not offered; the engine keeps that for the ability of a permanent.
    match p.get("ChoiceRestriction") {
        None => {}
        Some("ThisTurn") => modal["once_each_turn"] = json!(true),
        Some("ThisGame") => modal["once_ever"] = json!(true),
        Some(other) => return unmapped(format!("key:Charm.ChoiceRestriction={other}")),
    }
    if flag(p, "CanRepeatModes")? {
        modal["repeat"] = json!(true);
    }
    Ok(json!({ "Modal": modal }))
}

/// The cost a mode adds: mana, or the items of a cost.
fn cost_of(ctx: &Ctx, text: &str) -> Tr<Value> {
    super::cost::cost(ctx, text)
}

/// `DelayedTrigger` at a step ("at the beginning of the next end step, ..."): the part named by `Execute$` happens then.
fn delayed(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    // "Its controller" is named before the trigger is set up: the spell it is about is gone when the trigger fires.
    let mut out = Vec::new();
    let outer = ctx.delayed_remembered.take();
    // The delayed trigger line itself chooses a target: the event and the later lines read it as targeted.
    let own;
    let mut p = p;
    if p.has("ValidTgts") {
        super::valid::subject(ctx, p, "", None)?;
        own = p.without(&["ValidTgts", "TgtPrompt"]);
        p = &own;
    } else if p.get("RememberObjects") == Some("Targeted")
        && ctx.targets.is_empty()
        && ctx.coming.is_none()
    {
        // Nothing was targeted: the word names nobody, and a step's trigger needs no one.
        own = p.without(&["RememberObjects"]);
        p = &own;
    }
    // The object an event of this trigger is about, named now: the delayed trigger fires on another event.
    if let Some(word) = p
        .get("RememberObjects")
        .filter(|word| super::triggered::is_word(word))
    {
        let what = super::triggered::remembered_by_event(ctx.trigger.as_ref(), word)?;
        let name = ctx.fresh("event");
        out.push(json!({"Bind": {"what": what, "as": name}}));
        ctx.delayed_remembered = Some(json!({ "Bound": name }));
    }
    // A step has no event to name its target in: the lines it runs read the target as the trigger remembers it.
    if p.get("Mode") == Some("Phase") {
        let target = match p.get("RememberObjects") {
            Some("Targeted") => Some(ctx.last_target()?),
            Some("ParentTarget") => Some(ctx.parent()?),
            _ => None,
        };
        if let Some(what) = target {
            let name = ctx.fresh("target");
            out.push(json!({"Bind": {"what": what, "as": name}}));
            ctx.delayed_remembered = Some(json!({ "Bound": name }));
        }
    }
    if p.get("RememberObjects") == Some("RememberedController") {
        let who = super::memory::remembered_controller(ctx)?;
        let name = ctx.fresh("controller");
        out.push(json!({"Bind": {"what": who, "as": name}}));
        ctx.delayed_remembered = Some(json!({ "Bound": name }));
    }
    // A target the event waits for is the object the event is about when it fires.
    if matches!(p.get("RememberObjects"), Some("Targeted" | "ParentTarget"))
        && p.get("Mode") == Some("ChangesZone")
    {
        ctx.delayed_remembered = Some(json!("Triggering"));
    }
    let node = delayed_node(ctx, p);
    ctx.delayed_remembered = outer;
    out.push(node?);
    Ok(out)
}

fn delayed_node(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let Some(execute) = p.get("Execute") else {
        return unmapped("key:DelayedTrigger.no-Execute");
    };
    // The objects the trigger is about, which `Card.IsTriggerRemembered` in its event names.
    let pin = match p.get("RememberObjects") {
        None => None,
        Some("Remembered" | "RememberedLKI")
            if ctx
                .remembered
                .as_ref()
                .is_some_and(|held| !super::memory::is_scoped(ctx, held)) =>
        {
            ctx.remembered.clone()
        }
        Some("Targeted" | "ParentTarget") if p.get("Mode") == Some("Phase") => None,
        Some("Targeted") => Some(objects(ctx.last_target()?)?),
        Some("ParentTarget") => Some(objects(ctx.parent()?)?),
        Some("Self") => Some(json!("This")),
        Some(word) if super::triggered::is_word(word) => ctx.delayed_remembered.clone(),
        Some("RememberedController") if p.get("Mode") == Some("Phase") => None,
        Some(other) => return unmapped(format!("key:DelayedTrigger.RememberObjects={other}")),
    };
    let (when, this_turn) = if p.get("Mode") == Some("Phase") {
        // A step has no event to name the objects in; the lines after it read them as remembered.
        if let Some(other) = p.get("RememberObjects").filter(|word| {
            !matches!(
                *word,
                "Remembered"
                    | "RememberedLKI"
                    | "RememberedController"
                    | "Targeted"
                    | "ParentTarget"
            ) && !super::triggered::is_word(word)
        }) {
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
        to_player: false,
        pattern: super::triggered::key_of(&when),
        arrives: None,
    });
    let outer = std::mem::replace(&mut ctx.trigger, shape);
    let declared = ctx.targets.len();
    let effects = ctx
        .sub(execute)
        .and_then(|part| chain_at(ctx, &part, false));
    ctx.trigger = outer;
    let effects = later_targets(ctx, declared, effects?)?;
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

/// The targets a delayed trigger's own effects declare are chosen when it triggers, not when the spell is cast: they move
/// into a reflexive ability that has them for its own (`Target` counts from its first). An effect that also reads a target
/// of the spell cannot be split that way and stays a gap by name.
fn later_targets(ctx: &mut Ctx, declared: usize, effects: Vec<Value>) -> Tr<Vec<Value>> {
    if ctx.targets.len() == declared {
        return Ok(effects);
    }
    let own = ctx.targets.split_off(declared);
    let mut reads_outer = false;
    let moved: Vec<Value> = effects
        .into_iter()
        .map(|effect| rebase_targets(effect, declared, &mut reads_outer))
        .collect();
    if reads_outer {
        return unmapped("effect:DelayedTrigger-target-with-a-spell-target");
    }
    Ok(vec![
        json!({"Reflexive": {"targets": own, "effects": moved}}),
    ])
}

/// The value with every `{"Target": n}` from `first` on renumbered from zero; `outer` is set when one below `first` is read.
fn rebase_targets(value: Value, first: usize, outer: &mut bool) -> Value {
    match value {
        Value::Object(map) => {
            if let (1, Some(index)) = (map.len(), map.get("Target").and_then(Value::as_u64)) {
                return match usize::try_from(index) {
                    Ok(at) if at >= first => json!({"Target": at - first}),
                    _ => {
                        *outer = true;
                        Value::Object(map)
                    }
                };
            }
            Value::Object(
                map.into_iter()
                    .map(|(key, inner)| (key, rebase_targets(inner, first, outer)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|inner| rebase_targets(inner, first, outer))
                .collect(),
        ),
        other => other,
    }
}
