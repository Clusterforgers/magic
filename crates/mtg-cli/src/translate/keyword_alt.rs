//! Alternative-cost keywords: `Warp`, `Surge`, `Web-slinging` and `Impending` as an `AltCast` with the abilities that read
//! how the card was cast, the same shapes the engine's alternative-cost table runs (`mtg_cards::alt_riders`).

use mtg_cards::ability::Ability;
use mtg_cards::alt_riders::{alt_cast, impending_riders, sneak_riders, warp_riders};
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::mana::ManaCostX;
use mtg_cards::value::CastWay;
use serde_json::Value;

use super::ctx::{Tr, unmapped};

/// The abilities of a `K:` line of an alternative-cost keyword; `None` for any other keyword.
pub fn alt_keyword(body: &str) -> Option<Vec<Tr<Value>>> {
    let mut fields = body.splitn(3, ':');
    let name = fields.next()?;
    if name == "Strive" {
        return Some(vec![strive(fields.next(), fields.next())]);
    }
    if name == "Prototype" {
        return Some(vec![prototype(fields.next(), fields.next())]);
    }
    let (way, count, cost_text) = match (name, fields.next(), fields.next()) {
        ("Warp", Some(text), None) => (CastWay::Warp, None, text),
        ("Surge", Some(text), None) => (CastWay::Surge, None, text),
        ("Sneak", Some(text), None) => (CastWay::Sneak, None, text),
        ("Web-slinging", Some(text), None) => (CastWay::WebSlinging, None, text),
        ("Impending", Some(count), Some(text)) => (CastWay::Impending, Some(count), text),
        _ => return None,
    };
    Some(abilities(way, count, cost_text))
}

/// The mana of a cost word list such as `2 U U`; a word that is no mana symbol makes the cost a gap.
fn mana(text: &str) -> Option<ManaCostX> {
    let words: Vec<&str> = super::cost::parts(text);
    if words.is_empty() || !words.iter().all(|word| super::cost::is_mana(word)) {
        return None;
    }
    let mut symbols = String::new();
    for word in &words {
        symbols.push_str(&super::cost::symbol(word));
    }
    symbols.parse().ok()
}

fn abilities(way: CastWay, count: Option<&str>, cost_text: &str) -> Vec<Tr<Value>> {
    let Some(mana) = mana(cost_text) else {
        return vec![unmapped(format!("keyword:{way:?}-cost"))];
    };
    let mut all = vec![alt_cast(way, mana)];
    match way {
        CastWay::Warp => all.extend(warp_riders()),
        CastWay::Impending => match count.and_then(|n| n.parse::<u32>().ok()) {
            Some(n) => all.extend(impending_riders(n)),
            None => return vec![unmapped("keyword:Impending-count")],
        },
        CastWay::Sneak => all.extend(sneak_riders()),
        _ => {}
    }
    all.into_iter()
        .map(|ability| serde_json::to_value(ability).or_else(|_| unmapped("keyword:AltCast")))
        .collect()
}

/// `MayFlashCost:2`: the card may be cast as though it had flash if the caster pays the mana more (CR 601.3b, 118.8): a
/// way to cast it at instant speed for its own mana cost and the extra mana.
pub fn may_flash_cost(printed: &str, extra: &str) -> Tr<Value> {
    // An alternative way holds no {X} in the engine: such a card would never be offered at instant speed.
    let Some(total) = mana(&format!("{printed} {extra}"))
        .filter(|_| !printed.is_empty() && !printed.contains('X'))
    else {
        return unmapped("keyword:MayFlashCost-cost");
    };
    let ability = Ability::AltCast {
        way: CastWay::Alternative,
        cost: Some(Cost::mana(total)),
        from: mtg_cards::refs::ZoneKind::Hand,
        condition: None,
        flash: true,
    };
    serde_json::to_value(ability).or_else(|_| unmapped("keyword:MayFlashCost"))
}

/// `Prototype:1 U U:2:1`: the prototype mana cost, then the power and toughness.
fn prototype(cost_text: Option<&str>, size: Option<&str>) -> Tr<Value> {
    let (Some(text), Some(size)) = (cost_text, size) else {
        return unmapped("keyword:Prototype-shape");
    };
    let Some(mana) = mana(text) else {
        return unmapped("keyword:Prototype-cost");
    };
    let sized = size.split_once(':').and_then(|(power, toughness)| {
        Some((power.parse::<i32>().ok()?, toughness.parse::<i32>().ok()?))
    });
    let Some((power, toughness)) = sized else {
        return unmapped("keyword:Prototype-size");
    };
    let ability = Ability::Keyword(Keyword::Prototype {
        cost: Cost::mana(mana),
        power,
        toughness,
    });
    serde_json::to_value(ability).or_else(|_| unmapped("keyword:Prototype"))
}

/// `Strive`: this spell costs the mana more for each target beyond the first.
fn strive(cost_text: Option<&str>, rest: Option<&str>) -> Tr<Value> {
    let (Some(text), None) = (cost_text, rest) else {
        return unmapped("keyword:Strive-shape");
    };
    let Some(mana) = mana(text) else {
        return unmapped("keyword:Strive-cost");
    };
    let mana = serde_json::to_value(mana).or_else(|_| unmapped("keyword:Strive-cost"))?;
    Ok(serde_json::json!({"CostModifier": {
        "applies_to": ["This"],
        "change": "Increase",
        "mana": mana,
        "per": {"Sum": [{"TargetCount": 0}, -1]},
    }}))
}
