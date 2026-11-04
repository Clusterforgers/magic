//! Alternative-cost keywords: `Warp`, `Surge`, `Web-slinging` and `Impending` as an `AltCast` with the abilities that read
//! how the card was cast, the same shapes the engine's alternative-cost table runs (`mtg_cards::alt_riders`).

use std::fmt::Write as _;

use mtg_cards::alt_riders::{alt_cast, impending_riders, warp_riders};
use mtg_cards::mana::ManaCostX;
use mtg_cards::value::CastWay;
use serde_json::Value;

use super::ctx::{Tr, unmapped};

/// The abilities of a `K:` line of an alternative-cost keyword; `None` for any other keyword.
pub fn alt_keyword(body: &str) -> Option<Vec<Tr<Value>>> {
    let mut fields = body.splitn(3, ':');
    let name = fields.next()?;
    let (way, count, cost_text) = match (name, fields.next(), fields.next()) {
        ("Warp", Some(text), None) => (CastWay::Warp, None, text),
        ("Surge", Some(text), None) => (CastWay::Surge, None, text),
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
        let _ = write!(symbols, "{{{word}}}");
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
        _ => {}
    }
    all.into_iter()
        .map(|ability| serde_json::to_value(ability).or_else(|_| unmapped("keyword:AltCast")))
        .collect()
}
