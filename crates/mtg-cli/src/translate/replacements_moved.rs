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
    let valid = p.get("ValidCard").unwrap_or_default();
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
    let exiles = part.get("DB") == Some("ChangeZone")
        && part.get("Hidden") == Some("True")
        && part.get("Origin") == Some("All")
        && part.get("Destination") == Some("Exile")
        && part.get("Defined") == Some("ReplacedCard");
    if !exiles {
        return unmapped("replacement:Moved-not-an-exile");
    }
    Ok(json!({"Replacement": {"when": {"ZoneChange": pattern}, "by": {"MoveTo": "Exile"}}}))
}
