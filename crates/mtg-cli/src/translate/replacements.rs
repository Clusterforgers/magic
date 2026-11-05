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
        line_condition = Some(json!("Kicked"));
    }
    if event == "Untap" {
        return doesnt_untap(ctx, &p);
    }
    if event == "Counter" {
        if line_condition.is_some() {
            return unmapped("key:Replacement.Counter.condition");
        }
        return super::replacements_counter::counter(ctx, &p);
    }
    if matches!(event, "Draw" | "GainLife" | "AddCounter") {
        if line_condition.is_some() {
            return unmapped(format!("key:Replacement.{event}.condition"));
        }
        return match event {
            "Draw" => super::replacements_counter::draw(ctx, &p),
            "GainLife" => super::replacements_counter::gain_life(ctx, &p),
            _ => super::replacements_counter::add_counter(ctx, &p),
        };
    }
    if event == "LifeReduced" {
        return super::replacements_life::life_reduced(ctx, &p, line_condition.as_ref());
    }
    if event == "GameLoss" {
        return super::replacements_loss::game_loss(ctx, &p, line_condition.as_ref());
    }
    if event == "CreateToken" {
        if line_condition.is_some() {
            return unmapped("key:Replacement.CreateToken.condition");
        }
        return super::replacements_token::create_token(ctx, &p);
    }
    if event != "Moved" {
        return unmapped(format!("replacement:{event}"));
    }
    let read = [
        "Event",
        "ValidCard",
        "Destination",
        "ReplaceWith",
        "ReplacementResult",
        "ActiveZones",
        "Origin",
    ];
    check_keys(&p, "Replacement.Moved", &read, |_, _| false)?;
    if p.get("ReplacementResult")
        .is_some_and(|result| result != "Updated")
    {
        return unmapped("key:Replacement.Moved.ReplacementResult");
    }
    if p.get("ValidCard") != Some("Card.Self") && !kicked_self {
        return super::replacements_moved::others(ctx, &p, line_condition.as_ref());
    }
    if p.get("Destination") == Some("Graveyard") && !p.has("ActiveZones") {
        return super::replacements_moved::self_to_graveyard(ctx, &p, line_condition.as_ref());
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

/// "Doesn't untap during its controller's untap step": the untap that can't happen.
fn doesnt_untap(ctx: &Ctx, p: &Params) -> Tr<Value> {
    only_keys(
        p,
        &[
            "Event",
            "ValidCard",
            "ValidStepTurnToController",
            "Layer",
            "ActiveZones",
        ],
    )?;
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
    Ok(json!({"Static": {"affected": affected, "mods": ["DoesntUntap"]}}))
}

/// What the permanent does as it enters, from the part a replacement names.
fn enters_by(ctx: &Ctx, part: &Params) -> Tr<Value> {
    let api = part.get("DB").unwrap_or_default();
    if part.get("Defined") != Some("Self") || part.get("ETB") != Some("True") {
        return unmapped("replacement:Moved-not-an-entering-effect");
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
        "PutCounter" => {
            only_keys(part, &["DB", "Defined", "ETB", "CounterType", "CounterNum"])?;
            let Some(kind) = part.get("CounterType").and_then(counter_kind) else {
                return unmapped("key:Replacement.CounterType");
            };
            let count = amount(ctx, part.get("CounterNum").unwrap_or("1"))?;
            Ok(json!({"WithCounters": {"counter": kind, "count": count}}))
        }
        other => unmapped(format!("replacement:Moved-{other}")),
    }
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
            let mut selector = selector_any(ctx, what)?;
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

/// An `Effect` line whose replacement is "doesn't untap during your next untap step" for the card itself: the card
/// holds `DoesntUntap` through the next untap step of its controller.
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
        || p.get("ValidCard") != Some("Card.IsRemembered")
        || p.get("ValidStepTurnToController") != Some("Player.Activator")
    {
        return unmapped("replacement:Untap-shape");
    }
    if line.get("RememberObjects") != Some("Self")
        || line.get("Duration") != Some("UntilTheEndOfYourNextUntap")
        || line.has("StaticAbilities")
        || line.has("Triggers")
        || line.has("ValidTgts")
    {
        return unmapped("replacement:Untap-effect-shape");
    }
    Ok(vec![
        json!({"Continuous": {"affected": "This", "mods": ["DoesntUntap"], "duration": "NextUntapStep"}}),
    ])
}
