//! Zone changes: `ChangeZone` of a target or of the card itself, the library search, and `ChangeZoneAll`.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, only, unmapped};
use super::valid::{selector_any, selector_in, subject};
use crate::forge::script::Params;

/// The effects of `ChangeZone` and `ChangeZoneAll`.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    let origin = zone_word(p.get("Origin"), "Origin")?;
    let destination = zone_word(p.get("Destination"), "Destination")?;
    match (api, origin) {
        ("ChangeZone", "Library") => search(ctx, p, destination),
        ("ChangeZone", _) => change_zone(ctx, p, origin, destination),
        _ => change_zone_all(ctx, p, origin, destination),
    }
}

/// A zone word of one zone.
fn zone_word<'a>(word: Option<&'a str>, key: &str) -> Tr<&'a str> {
    match word {
        Some(zone @ ("Battlefield" | "Graveyard" | "Hand" | "Library" | "Exile")) => Ok(zone),
        Some(other) => unmapped(format!("zone:{key}={other}")),
        None => unmapped(format!("key:ChangeZone.no-{key}")),
    }
}

/// Where the moved objects go, as a destination or the exile verb.
enum Goes {
    Exile,
    To(Value),
}

fn goes(p: &Params, destination: &str) -> Tr<Goes> {
    let tapped = p.get("Tapped") == Some("True");
    Ok(match destination {
        "Exile" => Goes::Exile,
        "Hand" => Goes::To(json!("Hand")),
        "Graveyard" => Goes::To(json!("Graveyard")),
        "Battlefield" => Goes::To(json!({"Battlefield": {"tapped": tapped}})),
        "Library" => {
            let place = match p.get("LibraryPosition") {
                Some("0") => "Top",
                Some("-1") => "Bottom",
                _ => return unmapped("key:ChangeZone.LibraryPosition"),
            };
            Goes::To(json!({"Library": {"place": place}}))
        }
        other => return unmapped(format!("zone:Destination={other}")),
    })
}

fn moves(what: Value, goes: Goes) -> Value {
    match goes {
        Goes::Exile => json!({"Exile": {"what": what}}),
        Goes::To(to) => json!({"Move": {"what": what, "to": to}}),
    }
}

fn change_zone(ctx: &mut Ctx, p: &Params, origin: &str, destination: &str) -> Tr<Vec<Value>> {
    only(
        p,
        "ChangeZone",
        &[
            "Origin", "Destination", "ValidTgts", "Defined", "TargetMin", "TargetMax",
            "LibraryPosition", "Tapped",
        ],
    )?;
    let what = subject(ctx, p, "Self", Some(origin))?;
    Ok(vec![moves(what, goes(p, destination)?)])
}

/// The library search: the search, the move of what was found, then the shuffle.
fn search(ctx: &mut Ctx, p: &Params, destination: &str) -> Tr<Vec<Value>> {
    only(
        p,
        "ChangeZone.Search",
        &[
            "Origin", "Destination", "ChangeType", "ChangeNum", "Hidden", "Mandatory", "Tapped",
            "Reveal", "LibraryPosition",
        ],
    )?;
    if destination == "Library" {
        return unmapped("key:ChangeZone.Search-to-library");
    }
    let filter = selector_any(ctx, p.get("ChangeType").unwrap_or("Card"))?;
    let bind = ctx.fresh("found");
    let mut find = json!({"filter": filter, "as": bind});
    match p.get("ChangeNum") {
        None | Some("1") => {}
        Some(n) => find["count"] = json!({"min": 0, "max": amount(ctx, n)?}),
    }
    if p.get("Reveal") == Some("True") {
        find["reveal"] = json!(true);
    }
    let found = json!({ "Bound": bind });
    Ok(vec![
        json!({ "Search": find }),
        moves(found, goes(p, destination)?),
        json!({"Shuffle": {}}),
    ])
}

fn change_zone_all(
    ctx: &mut Ctx,
    p: &Params,
    origin: &str,
    destination: &str,
) -> Tr<Vec<Value>> {
    only(
        p,
        "ChangeZoneAll",
        &["Origin", "Destination", "ChangeType", "Tapped", "LibraryPosition"],
    )?;
    if origin == "Library" {
        return unmapped("key:ChangeZoneAll.Origin=Library");
    }
    let zone = (origin != "Battlefield").then_some(origin);
    let filter = selector_in(ctx, p.get("ChangeType").unwrap_or("Card"), zone)?;
    Ok(vec![moves(json!({ "Each": filter }), goes(p, destination)?)])
}
