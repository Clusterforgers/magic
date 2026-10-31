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
            let place = match (p.get("LibraryPosition"), p.get("Shuffle")) {
                (Some("0"), _) => "Top",
                (Some("-1"), _) => "Bottom",
                (None, Some("True")) => "Shuffled",
                _ => return unmapped("key:ChangeZone.LibraryPosition"),
            };
            Goes::To(json!({"Library": {"place": place}}))
        }
        other => return unmapped(format!("zone:Destination={other}")),
    })
}

fn moves(what: &Value, goes: Goes) -> Value {
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
            "Origin",
            "Destination",
            "ValidTgts",
            "Defined",
            "TargetMin",
            "TargetMax",
            "LibraryPosition",
            "Shuffle",
            "Tapped",
            "RememberChanged",
        ],
    )?;
    let what = subject(ctx, p, "Self", Some(origin))?;
    let mut effect = moves(&what, goes(p, destination)?);
    if p.get("RememberChanged") == Some("True") {
        // Only an exile binds the new objects; a move of a target does not name what it moved.
        if effect.get("Exile").is_none() {
            return unmapped("key:ChangeZone.RememberChanged-move");
        }
        let name = ctx.fresh("exiled");
        effect["Exile"]["as"] = json!(name);
        ctx.remembered = Some(json!({ "Bound": name }));
    }
    Ok(vec![effect])
}

/// The library search: the search, the move of what was found, then the shuffle.
fn search(ctx: &mut Ctx, p: &Params, destination: &str) -> Tr<Vec<Value>> {
    only(
        p,
        "ChangeZone.Search",
        &[
            "Origin",
            "Destination",
            "ChangeType",
            "ChangeNum",
            "Hidden",
            "Mandatory",
            "Tapped",
            "ShuffleNonMandatory",
            "RememberChanged",
            "Reveal",
            "LibraryPosition",
            "Shuffle",
            "Optional",
        ],
    )?;
    let filter = selector_any(ctx, p.get("ChangeType").unwrap_or("Card"))?;
    let filter_is_open = filter.is_empty();
    let bind = ctx.fresh("found");
    let mut find = json!({"filter": filter, "as": bind});
    match p.get("ChangeNum") {
        None | Some("1") => {}
        Some(n) => find["count"] = json!({"min": 0, "max": amount(ctx, n)?}),
    }
    if p.get("Reveal") == Some("True") || (destination == "Hand" && !filter_is_open) {
        find["reveal"] = json!(true);
    }
    let found = json!({ "Bound": bind });
    if p.get("RememberChanged") == Some("True") {
        ctx.remembered = Some(found.clone());
    }
    let search = json!({ "Search": find });
    let put = moves(&found, goes(p, destination)?);
    let shuffle = json!({"Shuffle": {}});
    let mut effects = match (destination, p.get("Shuffle")) {
        // The shuffle comes first when the card goes back on top: "then shuffle and put that card on top".
        ("Library", _) => vec![search, shuffle, put],
        (_, Some("False")) => vec![search, put],
        _ => vec![search, put, shuffle],
    };
    if p.get("Optional") == Some("True") {
        effects = vec![json!({"May": {"effects": effects}})];
    }
    Ok(effects)
}

/// `Dig`: look at the top cards, take some, put the rest somewhere ("look at the top four cards of your library; you may
/// reveal a creature card from among them and put it into your hand; put the rest on the bottom in any order").
pub fn dig(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Dig",
        &[
            "DigNum",
            "ChangeNum",
            "ChangeValid",
            "Optional",
            "Reveal",
            "DestinationZone",
            "DestinationZone2",
            "RestRandomOrder",
            "Defined",
            "Tapped",
            "NoReveal",
            "ForceRevealToController",
        ],
    )?;
    if !matches!(p.get("Defined"), None | Some("You")) {
        return unmapped("key:Dig.Defined");
    }
    let count = amount(ctx, p.get("DigNum").unwrap_or("1"))?;
    let top = ctx.fresh("top");
    let kept = ctx.fresh("kept");
    let rest = ctx.fresh("rest");
    let from_top = json!({ "Bound": top });
    let taken = match p.get("ChangeNum") {
        None | Some("1") => 1,
        Some(n) => n
            .parse::<u64>()
            .map_or_else(|_| unmapped("key:Dig.ChangeNum"), Ok)?,
    };
    let filter = p.get("ChangeValid");
    let may = p.get("Optional") == Some("True") || filter.is_some();
    let mut choose = match filter {
        Some(valid) => {
            let mut selector = vec![json!({"In": "Library"}), json!({"Is": from_top})];
            selector.extend(selector_any(ctx, valid)?);
            json!({"from": {"Each": selector}, "as": kept})
        }
        None => json!({"from": from_top, "as": kept, "rest_as": rest}),
    };
    if may || taken != 1 {
        let min = if may { 0 } else { taken };
        choose["count"] = json!({"min": min, "max": taken});
    }
    let to = match p.get("DestinationZone") {
        None | Some("Hand") => json!("Hand"),
        Some("Graveyard") => json!("Graveyard"),
        Some("Battlefield") => {
            json!({"Battlefield": {"tapped": p.get("Tapped") == Some("True")}})
        }
        Some(other) => return unmapped(format!("key:Dig.DestinationZone={other}")),
    };
    let order = if p.get("RestRandomOrder") == Some("True") {
        "Random"
    } else {
        "Chosen"
    };
    let rest_to = match p.get("DestinationZone2") {
        None => json!({"Library": {"place": "Bottom", "order": order}}),
        Some("Graveyard") => json!("Graveyard"),
        Some(other) => return unmapped(format!("key:Dig.DestinationZone2={other}")),
    };
    let rest_ref = if filter.is_some() {
        from_top.clone()
    } else {
        json!({ "Bound": rest })
    };
    let mut effects = vec![
        json!({"LookAtTop": {"count": count, "as": top}}),
        json!({ "Choose": choose }),
    ];
    if p.get("Reveal") == Some("True") {
        effects.push(json!({"Reveal": {"what": {"Bound": kept}}}));
    }
    effects.push(json!({"Move": {"what": {"Bound": kept}, "to": to}}));
    effects.push(json!({"Move": {"what": rest_ref, "to": rest_to}}));
    Ok(effects)
}

fn change_zone_all(ctx: &mut Ctx, p: &Params, origin: &str, destination: &str) -> Tr<Vec<Value>> {
    only(
        p,
        "ChangeZoneAll",
        &[
            "Origin",
            "Destination",
            "ChangeType",
            "Tapped",
            "LibraryPosition",
            "Shuffle",
        ],
    )?;
    if origin == "Library" {
        return unmapped("key:ChangeZoneAll.Origin=Library");
    }
    let zone = (origin != "Battlefield").then_some(origin);
    let filter = selector_in(ctx, p.get("ChangeType").unwrap_or("Card"), zone)?;
    Ok(vec![moves(
        &json!({ "Each": filter }),
        goes(p, destination)?,
    )])
}
