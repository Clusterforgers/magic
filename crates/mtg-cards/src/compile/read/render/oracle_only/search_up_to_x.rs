//! A search for up to X cards where X is a characteristic of the card: "search your library for up to X basic land
//! cards, where X is this creature's power, put them onto the battlefield, then shuffle."

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;
use serde_json::json;

/// The characteristic as the card words it.
fn stat_word(stat: &str) -> Option<&'static str> {
    match stat {
        "Power" => Some("power"),
        "Toughness" => Some("toughness"),
        "ManaValue" => Some("mana value"),
        _ => None,
    }
}

/// Adds the wordings of a search whose upper bound is a characteristic of the card.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut word = None;
    let plain = twin(ability, |value| {
        let effects = value
            .get_mut("Triggered")?
            .get_mut("effects")?
            .as_array_mut()?;
        for effect in effects {
            let Some(max) = effect
                .get_mut("Search")
                .and_then(|search| search.get_mut("count")?.get_mut("max"))
            else {
                continue;
            };
            let stat = max.get("Stat")?;
            (stat.get("of")? == "This").then_some(())?;
            word = stat_word(stat.get("stat")?.as_str()?);
            *max = json!(2);
            return Some(());
        }
        None::<()>
    });
    let (Some(plain), Some(word)) = (plain, word) else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter_map(|text| {
            let text = text.replacen("up to two ", "up to X ", 1);
            let (head, tail) = text.split_once(", put ")?;
            Some(format!("{head}, where X is ~'s {word}, put {tail}"))
        })
        .collect();
    texts.extend(said);
}
