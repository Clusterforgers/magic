//! `Event$ Moved` replacements that act on other cards while their source is on the battlefield: permanents that enter
//! tapped, and cards that go to a graveyard and are exiled instead.

use serde_json::{Value, json};

use super::condition::prefixed;
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The `Replacement` ability of a `Moved` line whose `ValidCard$` is not the card itself.
pub fn others(ctx: &mut Ctx, p: &Params, line_condition: Option<&Value>) -> Tr<Value> {
    if line_condition.is_some() {
        return unmapped("key:Replacement.Moved.condition");
    }
    if p.get("ActiveZones") != Some("Battlefield") {
        return unmapped("key:Replacement.Moved.ActiveZones");
    }
    // `ValidLKI$` names the object as it was before it moved, which is how a zone-change pattern reads it.
    let valid = p
        .get("ValidCard")
        .or_else(|| p.get("ValidLKI"))
        .unwrap_or_default();
    if valid.is_empty() || valid.contains("Self") {
        return unmapped("replacement:Moved-shape");
    }
    let what = selector_any(ctx, valid)?;
    let Some(name) = p.get("ReplaceWith") else {
        return unmapped("replacement:Moved-without-ReplaceWith");
    };
    let raw = ctx.sub(name)?;
    let (part_condition, part) = prefixed(ctx, &raw)?;
    if part_condition.is_some() {
        return unmapped("key:Replacement.Moved.condition");
    }
    match p.get("Destination") {
        Some("Battlefield") => enter_tapped(p, &part, &what),
        Some("Graveyard") => exiled_instead(p, &part, &what),
        _ => unmapped("replacement:Moved-shape"),
    }
}

/// "Creatures your opponents control enter tapped": the entering permanent is tapped.
fn enter_tapped(p: &Params, part: &Params, what: &[Value]) -> Tr<Value> {
    if p.has("Origin") {
        return unmapped("key:Replacement.Moved.Origin");
    }
    check_keys(part, "Replacement", &["DB", "Defined", "ETB"], |_, _| false)?;
    if part.get("DB") != Some("Tap")
        || part.get("Defined") != Some("ReplacedCard")
        || part.get("ETB") != Some("True")
    {
        return unmapped("replacement:Moved-not-an-entering-effect");
    }
    Ok(json!({"Replacement": {"when": {"Enters": {"what": what}}, "by": "Tapped"}}))
}

/// "If a card would be put into a graveyard, exile it instead": from the battlefield when `Origin$` says so, from anywhere
/// otherwise.
fn exiled_instead(p: &Params, part: &Params, what: &[Value]) -> Tr<Value> {
    let mut pattern = json!({"what": what, "to": "Graveyard"});
    match p.get("Origin") {
        None => {}
        Some("Battlefield") => pattern["from"] = json!("Battlefield"),
        Some(_) => return unmapped("key:Replacement.Moved.Origin"),
    }
    check_keys(
        part,
        "Replacement",
        &["DB", "Hidden", "Origin", "Destination", "Defined"],
        |_, _| false,
    )?;
    // A part that leaves the battlefield names it, which is what the replaced move is when the line says nothing.
    if part.get("Origin") == Some("Battlefield") {
        pattern["from"] = json!("Battlefield");
    }
    let to = match part.get("Destination") {
        Some("Exile") => "Exile",
        Some("Hand") if pattern.get("from").is_some() => "Hand",
        _ => return unmapped("replacement:Moved-not-an-exile"),
    };
    let moves = part.get("DB") == Some("ChangeZone")
        && matches!(part.get("Hidden"), None | Some("True"))
        && matches!(part.get("Origin"), Some("All" | "Battlefield"))
        && part.get("Defined") == Some("ReplacedCard");
    if !moves {
        return unmapped("replacement:Moved-not-an-exile");
    }
    Ok(json!({"Replacement": {"when": {"ZoneChange": pattern}, "by": {"MoveTo": to}}}))
}

/// The shield an `Effect` line makes from an `Event$ Moved` replacement over a class of cards: those put into a graveyard
/// and exiled instead ("if a creature would die this turn"), or permanents that enter tapped ("permanents enter tapped this
/// turn"). Each move is judged as it happens. A line that names particular objects (`IsRemembered`) is a gap.
pub fn shield(ctx: &mut Ctx, p: &Params, duration: &str) -> Tr<Vec<Value>> {
    let valid = p
        .get("ValidCard")
        .or_else(|| p.get("ValidLKI"))
        .unwrap_or_default();
    let objects = ["Self", "Remembered", "Chosen", "CastSa", "wasCast"];
    if valid.is_empty() || objects.iter().any(|word| valid.contains(word)) {
        return unmapped("replacement:Moved-shield-objects");
    }
    let read = [
        "Event",
        "ValidCard",
        "ValidLKI",
        "Destination",
        "Origin",
        "ReplaceWith",
        "ReplacementResult",
        "ActiveZones",
    ];
    check_keys(p, "Replacement.Moved", &read, |_, _| false)?;
    // The effect object lives in the command zone and the replaced card is the updated one: neither changes the shield.
    if p.get("ActiveZones").is_some_and(|zone| zone != "Command")
        || p.get("ReplacementResult")
            .is_some_and(|result| result != "Updated")
    {
        return unmapped("replacement:Moved-shield");
    }
    let what = selector_any(ctx, valid)?;
    let Some(name) = p.get("ReplaceWith") else {
        return unmapped("replacement:Moved-without-ReplaceWith");
    };
    let raw = ctx.sub(name)?;
    let (part_condition, part) = prefixed(ctx, &raw)?;
    if part_condition.is_some() {
        return unmapped("key:Replacement.Moved.condition");
    }
    let replacement = match p.get("Destination") {
        Some("Graveyard") => exiled_instead(p, &part, &what)?,
        Some("Battlefield") => enter_tapped(p, &part, &what)?,
        _ => return unmapped("replacement:Moved-shield"),
    };
    Ok(vec![json!({"Shield": {
        "when": replacement["Replacement"]["when"],
        "by": replacement["Replacement"]["by"],
        "duration": duration,
    }})])
}

/// "If this would be put into a graveyard from anywhere, exile it instead": the card itself, in every zone.
pub fn self_to_graveyard(ctx: &mut Ctx, p: &Params, line_condition: Option<&Value>) -> Tr<Value> {
    if line_condition.is_some() {
        return unmapped("key:Replacement.Moved.condition");
    }
    if p.has("Origin") {
        return unmapped("key:Replacement.Moved.Origin");
    }
    let Some(name) = p.get("ReplaceWith") else {
        return unmapped("replacement:Moved-without-ReplaceWith");
    };
    let raw = ctx.sub(name)?;
    let (part_condition, part) = prefixed(ctx, &raw)?;
    if part_condition.is_some() {
        return unmapped("key:Replacement.Moved.condition");
    }
    exiled_instead(p, &part, &[json!("This")])
}
