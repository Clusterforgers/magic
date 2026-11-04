//! The ability header: trigger words the header grammar reads, and the keys every API shares (the zone of a target).

use serde_json::Value;

use super::ctx::{Ctx, Tr};
use crate::forge::script::Params;

/// The trigger pattern of a `Mode$` word read here; `None` until a word is read.
pub fn trigger_words(_ctx: &Ctx, _p: &Params, _mode: &str) -> Option<Tr<Value>> {
    None
}

/// The line with some pairs replaced by new values and some dropped; the order of the keys stays.
fn rebuilt(p: &Params, replace: &[(&str, String)], drop: &[&str]) -> Params {
    let body: Vec<String> = p
        .iter()
        .filter(|(key, _)| !drop.contains(key))
        .map(|(key, value)| {
            let value = replace
                .iter()
                .find(|(name, _)| *name == key)
                .map_or(value, |(_, new)| new.as_str());
            format!("{key}$ {value}")
        })
        .collect();
    Params::parse(&body.join(" | "))
}

/// The value of a key without counting it as read.
fn peek<'a>(p: &'a Params, key: &str) -> Option<&'a str> {
    p.iter().find(|(name, _)| *name == key).map(|(_, v)| v)
}

/// The zones `TgtZone$` names, `None` for a zone the DSL's selectors do not know.
fn target_zones(text: &str) -> Option<Vec<&str>> {
    let zones: Vec<&str> = text.split(',').collect();
    zones
        .iter()
        .all(|zone| {
            matches!(
                *zone,
                "Battlefield" | "Graveyard" | "Hand" | "Library" | "Exile" | "Stack"
            )
        })
        .then_some(zones)
}

/// One alternative of `ValidTgts$` for the cards of `zone`: the zone as a property, and control as ownership outside the
/// stack and the battlefield. `None` for a player word, a `Permanent` (a card of a permanent type off the battlefield)
/// and a control property that ownership cannot stand for.
fn in_zone(alternative: &str, zone: &str, extra: Option<&str>) -> Option<String> {
    let (base, props) = alternative.split_once('.').unwrap_or((alternative, ""));
    if matches!(base, "Any" | "Player" | "Opponent" | "Permanent") {
        return None;
    }
    let owned = !matches!(zone, "Battlefield" | "Stack");
    let mut all: Vec<String> = Vec::new();
    for prop in props
        .split('+')
        .chain(extra)
        .filter(|prop| !prop.is_empty())
    {
        all.push(match prop {
            "YouCtrl" if owned => "YouOwn".to_owned(),
            "OppCtrl" if owned => "OppOwn".to_owned(),
            _ if owned && prop.contains("Ctrl") => return None,
            other => other.to_owned(),
        });
    }
    if zone != "Battlefield" {
        all.push(format!("inZone{zone}"));
    }
    Some(format!("{base}.{}", all.join("+")))
}

/// `TgtZone$` (and `TargetType$ Spell` beside `TgtZone$ Stack`) folded into the target's valid-string as `inZone` properties,
/// so the mapper of the verb sees a target of those cards. `None` leaves the line as it is: a zone the mapper reads itself
/// (`Origin$` equal to the zone; `Counter` and the copies of spells read `TargetType$`), or a target the zone cannot state.
pub fn zone_targets(p: &Params) -> Option<Params> {
    let zone_text = peek(p, "TgtZone")?;
    let valid = peek(p, "ValidTgts")?;
    let zones = target_zones(zone_text)?;
    let api = ["SP", "AB", "DB"].iter().find_map(|head| peek(p, head))?;
    if matches!(api, "Counter" | "CopySpellAbility" | "ChangeTargets") {
        return None;
    }
    if let Some(origin) = peek(p, "Origin") {
        return (origin == zone_text && peek(p, "TargetType").is_none())
            .then(|| rebuilt(p, &[], &["TgtZone"]));
    }
    let extra = match peek(p, "TargetType") {
        None => None,
        Some("Spell") if zone_text == "Stack" => None,
        Some("Spell.OppCtrl") if zone_text == "Stack" => Some("OppCtrl"),
        Some("Spell.YouCtrl") if zone_text == "Stack" => Some("YouCtrl"),
        Some(_) => return None,
    };
    let mut out = Vec::new();
    for alternative in valid.split(',') {
        for zone in &zones {
            out.push(in_zone(alternative.trim(), zone, extra)?);
        }
    }
    Some(rebuilt(
        p,
        &[("ValidTgts", out.join(","))],
        &["TgtZone", "TargetType"],
    ))
}
