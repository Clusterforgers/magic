//! Replacement effects: an `R:` line as the DSL's `Replacement` ability (a permanent that enters tapped or with counters).

use serde_json::{Value, json};

use super::amount::{amount, counter_kind};
use super::condition::{prefixed, tapped_when};
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// A replacement line as a `Replacement` ability.
pub fn replacement_line(ctx: &mut Ctx, body: &str) -> Tr<Value> {
    let mut found = replacement_lines(ctx, body)?;
    if found.len() == 1 {
        return Ok(found.remove(0));
    }
    unmapped("replacement:several-abilities")
}

/// A replacement line as the `Replacement` abilities it stands for (a list of recipients is one for each).
pub fn replacement_lines(ctx: &mut Ctx, body: &str) -> Tr<Vec<Value>> {
    let all = Params::parse(body);
    if all.get("Event") == Some("DamageDone") {
        ctx.reset();
        let (line_condition, p) = prefixed(ctx, &all)?;
        if line_condition.is_some() {
            return unmapped("key:Replacement.DamageDone.condition");
        }
        return super::replacements_damage::static_prevention(ctx, &p);
    }
    if all.get("Event") == Some("LifeReduced") {
        ctx.reset();
        let (condition, p) = prefixed(ctx, &all)?;
        return super::replacements_life::life_reduced(ctx, &p, condition.as_ref())
            .map(|ability| vec![ability]);
    }
    single(ctx, body).map(|ability| vec![ability])
}

fn single(ctx: &mut Ctx, body: &str) -> Tr<Value> {
    ctx.reset();
    let all = Params::parse(body);
    let (mut line_condition, p) = prefixed(ctx, &all)?;
    let event = p.get("Event").unwrap_or_default();
    // "If it was kicked, it enters with ...": the card itself, and the replacement applies only to a kicked spell.
    let kicked_self = event == "Moved" && p.get("ValidCard") == Some("Card.Self+kicked");
    if kicked_self {
        if line_condition.is_some() {
            return unmapped("key:Replacement.two-conditions");
        }
        if animates(ctx, &p) {
            return kicked_with_changes(ctx, &p);
        }
        line_condition = Some(json!("Kicked"));
    }
    if event != "Moved" {
        return not_moved(ctx, &p, event, line_condition.as_ref());
    }
    if let Some(found) = super::e15b_entering::day_if_neither(ctx, &p, line_condition.as_ref()) {
        return found;
    }
    let read = [
        "Event",
        "ValidCard",
        "Destination",
        "ReplaceWith",
        "ReplacementResult",
        "ActiveZones",
        "ValidLKI",
        "Origin",
        "IsPresent",
    ];
    check_keys(&p, "Replacement.Moved", &read, |_, _| false)?;
    // "As long as this is tapped, other permanents enter tapped": the source's own state is the condition.
    if let Some(valid) = p.get("IsPresent") {
        let tapped = json!({"Matches": {"subject": "This", "selector": ["Tapped"]}});
        let state = match valid {
            "Card.Self+tapped" => tapped,
            "Card.Self+untapped" => json!({"Not": tapped}),
            other => return unmapped(format!("key:Replacement.Moved.IsPresent={other}")),
        };
        if line_condition.is_some() || p.get("ValidCard") == Some("Card.Self") {
            return unmapped("key:Replacement.two-conditions");
        }
        line_condition = Some(state);
    }
    if p.get("ReplacementResult")
        .is_some_and(|result| result != "Updated")
    {
        return unmapped("key:Replacement.Moved.ReplacementResult");
    }
    if (p.get("ValidCard") != Some("Card.Self") || p.has("ValidLKI")) && !kicked_self {
        return super::replacements_moved::others(ctx, &p, line_condition.as_ref());
    }
    if let Some(found) = super::replacements_moved::self_moved(ctx, &p, line_condition.as_ref()) {
        return found;
    }
    if let Some(found) =
        super::replacements_enter::e14b_pay_or_graveyard(ctx, &p, line_condition.as_ref())
    {
        return found;
    }
    if p.has("ActiveZones") {
        return unmapped("key:Replacement.Moved.ActiveZones");
    }
    if p.has("Origin") {
        return unmapped("key:Replacement.Moved.Origin");
    }
    if p.get("Destination") != Some("Battlefield") {
        return unmapped("replacement:Moved-shape");
    }
    let Some(name) = p.get("ReplaceWith") else {
        return unmapped("replacement:Moved-without-ReplaceWith");
    };
    let raw = ctx.sub(name)?;
    let (part_condition, part) = prefixed(ctx, &raw)?;
    if part.get("DB") == Some("Reveal") {
        if line_condition.is_some() || part_condition.is_some() {
            return unmapped("key:Replacement.two-conditions");
        }
        let (by, condition) = super::replacements_enter::reveal_or_tapped(ctx, &part)?;
        return Ok(
            json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": by, "condition": condition}}),
        );
    }
    let by = enters_by(ctx, &part)?;
    let mut ability = json!({"when": {"Enters": {"what": ["This"]}}, "by": by});
    let conditions: Vec<Value> = [line_condition, part_condition]
        .into_iter()
        .flatten()
        .collect();
    match conditions.as_slice() {
        [] => {}
        [only] => ability["condition"] = tapped_when(only.clone()),
        _ => return unmapped("key:Replacement.two-conditions"),
    }
    Ok(json!({ "Replacement": ability }))
}

/// A replacement line of an event other than a zone move.
fn not_moved(ctx: &mut Ctx, p: &Params, event: &str, condition: Option<&Value>) -> Tr<Value> {
    match event {
        "Counter" | "Draw" | "GainLife" | "AddCounter" | "CreateToken" if condition.is_some() => {
            unmapped(format!("key:Replacement.{event}.condition"))
        }
        "Destroy" if condition.is_none() => super::replacements_moved::destroyed(ctx, p),
        "Counter" => super::replacements_counter::counter(ctx, p),
        "Draw" => super::replacements_counter::draw(ctx, p),
        "GainLife" => super::replacements_counter::gain_life(ctx, p),
        "AddCounter" => super::replacements_counter::add_counter(ctx, p),
        "ProduceMana" => super::replacements_mana::produce_mana(ctx, p, condition),
        "Untap" => doesnt_untap(ctx, p),
        "BeginPhase" => super::replacements_phase::skip_step(p, condition),
        "GameLoss" => super::replacements_loss::game_loss(ctx, p, condition),
        "BeginTurn" => super::statics_rules::skip_extra_turns(p),
        "CreateToken" => super::replacements_token::create_token(ctx, p),
        "TurnFaceUp" => super::e14f_turn_up::turn_up(ctx, p, condition),
        _ => unmapped(format!("replacement:{event}")),
    }
}

/// Whether the chain the line replaces with gives the permanent something for good (`Animate`), read as the
/// `ETBReplacement` keyword reads it.
fn animates(ctx: &Ctx, p: &Params) -> bool {
    let mut next = p.get("ReplaceWith").map(str::to_owned);
    for _ in 0..4 {
        let Some(name) = next.take() else {
            return false;
        };
        let Some(part) = ctx.svar(&name).map(Params::parse) else {
            return false;
        };
        if part.get("DB") == Some("Animate") {
            return true;
        }
        next = part.get("SubAbility").map(str::to_owned);
    }
    false
}

/// "If it was kicked, it enters with two +1/+1 counters on it and with haste" as an `R:` line: the counters and the
/// changes of the chain, as the `ETBReplacement` keyword reads them.
fn kicked_with_changes(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let read = [
        "Event",
        "ValidCard",
        "Destination",
        "ReplaceWith",
        "ReplacementResult",
    ];
    check_keys(p, "Replacement.Moved", &read, |_, _| false)?;
    if p.get("Destination") != Some("Battlefield") || p.get("ReplacementResult") != Some("Updated")
    {
        return unmapped("replacement:Moved-shape");
    }
    let name = p.get("ReplaceWith").unwrap_or_default().to_owned();
    super::enters_with::self_enters(ctx, &name, "Card.Self+kicked")
}

/// "Doesn't untap during its controller's untap step": the untap that can't happen.
fn doesnt_untap(ctx: &Ctx, p: &Params) -> Tr<Value> {
    // "...if you control a Reflection" (`IsPresent$`) is the condition of the static ability.
    let held = super::condition::condition(ctx, p)?;
    let p = &super::condition::strip(p);
    only_keys(
        p,
        &[
            "Event",
            "ValidCard",
            "ValidStepTurnToController",
            "Layer",
            "ActiveZones",
            // Only a hint to the computer player (`SVar:DestroyWhenDamaged`); the script rule is in the trigger.
            "AddSVar",
        ],
    )?;
    let _ = p.get("AddSVar");
    if p.get("Layer") != Some("CantHappen") || p.get("ValidStepTurnToController") != Some("You") {
        return unmapped("replacement:Untap-shape");
    }
    if !matches!(p.get("ActiveZones"), None | Some("Battlefield")) {
        return unmapped("replacement:Untap-zone");
    }
    let affected = match p.get("ValidCard") {
        Some("Card.Self" | "Creature.Self") => json!("This"),
        Some(
            "Creature.EnchantedBy"
            | "Card.EnchantedBy"
            | "Permanent.EnchantedBy"
            | "Creature.AttachedBy"
            | "Permanent.AttachedBy"
            | "Creature.EquippedBy"
            | "Card.EquippedBy",
        ) => json!("Attached"),
        // A group ("Creatures don't untap during their controllers' untap steps") is each permanent it selects.
        Some(valid)
            if (!valid.contains("Self") || valid.starts_with("Card.Self+counters_"))
                && !valid.contains("Chosen")
                && p.get("ActiveZones") == Some("Battlefield") =>
        {
            json!({"Each": selector_any(ctx, valid)?})
        }
        _ => return unmapped("replacement:Untap-affected"),
    };
    let mut ability = json!({"Static": {"affected": affected, "mods": ["DoesntUntap"]}});
    if let Some(held) = held {
        ability["Static"]["condition"] = held;
    }
    Ok(ability)
}

/// What the permanent does as it enters, from the part a replacement names.
fn enters_by(ctx: &mut Ctx, part: &Params) -> Tr<Value> {
    let api = part.get("DB").unwrap_or_default();
    if part.get("Defined") != Some("Self") || part.get("ETB") != Some("True") {
        return unmapped("replacement:Moved-not-an-entering-effect");
    }
    if let Some(next) = part.get("SubAbility") {
        return tapped_with_counters(ctx, part, api, next);
    }
    match api {
        "Tap" => {
            only_keys(part, &["DB", "Defined", "ETB", "UnlessCost", "UnlessPayer"])?;
            match (part.get("UnlessCost"), part.get("UnlessPayer")) {
                (None, None) => Ok(json!("Tapped")),
                (Some(unless), Some("You")) => pay_or_tapped(ctx, unless),
                _ => unmapped("key:Replacement.UnlessCost-shape"),
            }
        }
        "PutCounter" => counters(ctx, part),
        other => unmapped(format!("replacement:Moved-{other}")),
    }
}

/// `PutCounter` as it enters: the counters the permanent enters with.
fn counters(ctx: &Ctx, part: &Params) -> Tr<Value> {
    only_keys(
        part,
        &[
            "DB",
            "Defined",
            "ETB",
            "CounterType",
            "CounterNum",
            "SubAbility",
        ],
    )?;
    let Some(kind) = part.get("CounterType").and_then(counter_kind) else {
        return unmapped("key:Replacement.CounterType");
    };
    let count = amount(ctx, part.get("CounterNum").unwrap_or("1"))?;
    Ok(json!({"WithCounters": {"counter": kind, "count": count}}))
}

/// "Enters tapped with two depletion counters": a `Tap` and a `PutCounter` of the entering permanent, in either order
/// (CR 614.1c: both change how it enters).
fn tapped_with_counters(ctx: &mut Ctx, part: &Params, api: &str, next: &str) -> Tr<Value> {
    let other = ctx.sub(next)?;
    let (tap, put) = match (api, other.get("DB")) {
        ("Tap", Some("PutCounter")) => (part, &other),
        ("PutCounter", Some("Tap")) => (&other, part),
        _ => return unmapped("replacement:Moved-not-an-entering-effect"),
    };
    if other.get("Defined") != Some("Self")
        || other.get("ETB") != Some("True")
        || other.has("SubAbility")
    {
        return unmapped("replacement:Moved-not-an-entering-effect");
    }
    only_keys(tap, &["DB", "Defined", "ETB", "SubAbility"])?;
    let mut by = counters(ctx, put)?;
    by["WithCounters"]["tapped"] = json!(true);
    Ok(by)
}

/// "As it enters, you may pay 2 life. If you don't, it enters tapped" and "...reveal a Giant card from your hand".
fn pay_or_tapped(ctx: &Ctx, unless: &str) -> Tr<Value> {
    let Some((head, rest)) = unless.split_once('<') else {
        return unmapped("key:Replacement.UnlessCost-shape");
    };
    let Some(inner) = rest.strip_suffix('>') else {
        return unmapped("key:Replacement.UnlessCost-shape");
    };
    let (number, what) = inner.split_once('/').unwrap_or((inner, ""));
    match head {
        "PayLife" => match number.parse::<u32>() {
            Ok(life) => Ok(json!({ "PayLifeOrTapped": life })),
            Err(_) => unmapped("key:Replacement.UnlessCost-life"),
        },
        "Reveal" if number == "1" => {
            // `Reveal<1/Island;Mountain/Island or Mountain>`: the types, then the words a player reads.
            let types = what.split('/').next().unwrap_or(what).replace(';', ",");
            let mut selector = selector_any(ctx, &types)?;
            selector.push(json!({"In": "Hand"}));
            selector.push(json!({"OwnedBy": "You"}));
            Ok(json!({ "RevealOrTapped": selector }))
        }
        other => unmapped(format!("key:Replacement.UnlessCost={other}")),
    }
}

fn only_keys(part: &Params, read: &[&str]) -> Tr<()> {
    check_keys(part, "Replacement", read, |_, _| false)
}

/// An `Effect` line whose replacement is an untap that does not happen for a while (CR 502.3): the card itself or the
/// lands of the controller through their next untap step, or the objects the line remembers for as long as its duration.
pub fn stays_tapped(ctx: &mut Ctx, line: &Params) -> Tr<Vec<Value>> {
    let Some(name) = line.get("ReplacementEffects") else {
        return unmapped("key:Effect.no-ReplacementEffects");
    };
    let p = ctx.sub(name.trim())?;
    only_keys(
        &p,
        &[
            "Event",
            "ValidCard",
            "ValidStepTurnToController",
            "Layer",
            "Description",
        ],
    )?;
    if p.get("Layer") != Some("CantHappen")
        || line.has("StaticAbilities")
        || line.has("Triggers")
        || (line.has("ValidTgts") && !line.has("RememberObjects"))
    {
        return unmapped("replacement:Untap-effect-shape");
    }
    if line.has("RememberObjects") {
        return remembered_stays_tapped(ctx, line, &p);
    }
    // "Lands you control don't untap during your next untap step": the group is read when the untap step comes.
    let valid = p.get("ValidCard").unwrap_or_default();
    if line.get("Duration") != Some("UntilTheEndOfYourNextUntap")
        || p.get("ValidStepTurnToController") != Some("Player.Activator")
        || valid.is_empty()
        || valid.contains("Remembered")
        || valid.contains("Self")
    {
        return unmapped("replacement:Untap-effect-shape");
    }
    Ok(vec![
        json!({"Continuous": {"affected": {"Each": selector_any(ctx, valid)?}, "mods": ["DoesntUntap"], "duration": "NextUntapStep"}}),
    ])
}

/// The objects an `Effect` line remembers don't untap during their controllers' untap steps, or the source itself
/// through the controller's next untap step.
fn remembered_stays_tapped(ctx: &mut Ctx, line: &Params, p: &Params) -> Tr<Vec<Value>> {
    if p.get("ValidCard") != Some("Card.IsRemembered") {
        return unmapped("replacement:Untap-effect-shape");
    }
    if line.get("RememberObjects") == Some("Self")
        && line.get("Duration") == Some("UntilTheEndOfYourNextUntap")
    {
        if p.get("ValidStepTurnToController") != Some("Player.Activator") || line.has("ValidTgts") {
            return unmapped("replacement:Untap-effect-shape");
        }
        return Ok(vec![
            json!({"Continuous": {"affected": "This", "mods": ["DoesntUntap"], "duration": "NextUntapStep"}}),
        ]);
    }
    // Without the step word the rule is "can't become untapped" at all, which an untap effect would break.
    if p.get("ValidStepTurnToController") != Some("You") {
        return unmapped("replacement:Untap-shape");
    }
    let duration = super::effects_effect::lasting(line)?;
    let who = super::effects_effect::remembered(ctx, line)?;
    Ok(vec![
        json!({"Continuous": {"affected": who, "mods": ["DoesntUntap"], "duration": duration}}),
    ])
}
