//! The ability header: trigger words the header grammar reads, and the keys every API shares (the zone of a target).

use serde_json::{Value, json};

use super::condition::comparison;
use super::ctx::{Ctx, Tr, unmapped};
use crate::forge::script::Params;

/// The trigger pattern of a `Mode$` word read here; `None` until a word is read.
pub fn trigger_words(_ctx: &Ctx, _p: &Params, _mode: &str) -> Option<Tr<Value>> {
    None
}

/// The keys of [`gate_conditions`].
pub const GATE_KEYS: [&str; 6] = [
    "ConditionPlayerTurn",
    "ConditionManaSpent",
    "ConditionManaNotSpent",
    "ConditionLifeTotal",
    "ConditionLifeAmount",
    "ConditionNotPresent",
];

/// The mana type a color letter names.
fn mana_type(letter: &str) -> Option<&'static str> {
    Some(match letter {
        "W" => "White",
        "U" => "Blue",
        "B" => "Black",
        "R" => "Red",
        "G" => "Green",
        "C" => "Colorless",
        _ => return None,
    })
}

/// The conditions the gate keys of a chain line state: whose turn it is, which mana was spent on the spell, and how the life
/// totals compare (`ConditionNotPresent$` is read by [`super::condition::prefixed`]).
pub fn gate_conditions(ctx: &Ctx, p: &Params) -> Tr<Vec<Value>> {
    let mut all = Vec::new();
    match p.get("ConditionPlayerTurn") {
        None => {}
        Some("True") => all.push(json!("YourTurn")),
        Some("False") => all.push(json!({"Not": "YourTurn"})),
        Some(other) => return unmapped(format!("condition:PlayerTurn={other}")),
    }
    if let Some(text) = p.get("ConditionManaSpent") {
        let mut spent = Vec::new();
        for letter in text.split_whitespace() {
            let Some(kind) = mana_type(letter) else {
                return unmapped(format!("condition:ManaSpent={letter}"));
            };
            spent.push(json!({ "ManaSpent": kind }));
        }
        match spent.len() {
            0 => return unmapped("condition:ManaSpent="),
            1 => all.append(&mut spent),
            _ => all.push(json!({ "All": spent })),
        }
    }
    if let Some(text) = p.get("ConditionManaNotSpent") {
        let Some(kind) = mana_type(text) else {
            return unmapped(format!("condition:ManaNotSpent={text}"));
        };
        all.push(json!({"Not": {"ManaSpent": kind}}));
    }
    match (p.get("ConditionLifeTotal"), p.get("ConditionLifeAmount")) {
        (None, None) => {}
        (Some("You"), Some(compare)) => {
            let (op, right) = comparison(ctx, compare)?;
            all.push(json!({"Compare": {"left": {"Life": "You"}, "op": op, "right": right}}));
        }
        (Some(other), _) => return unmapped(format!("condition:LifeTotal={other}")),
        (None, Some(_)) => return unmapped("condition:LifeAmountWithoutLifeTotal"),
    }
    Ok(all)
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
