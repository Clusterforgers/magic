//! A search of your library and graveyard (r7c): "search your library and/or graveyard for a card named Ashiok,
//! Sculptor of Fears, reveal it, and put it into your hand. If you search your library this way, shuffle." It is the
//! search of the library alone with the graveyard as a second zone and the shuffle only after a search of the library.

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;
use serde_json::{Value, json};

/// The shuffle the card asks for only when the library was searched.
fn shuffle_if_searched() -> Value {
    json!({ "If": { "condition": { "Searched": { "zone": "Library" } }, "then": [{ "Shuffle": {} }] } })
}

/// The steps with the search of library and graveyard made a search of the library alone, its shuffle unconditional.
fn plain_steps(steps: &mut [Value]) -> Option<()> {
    let at = steps.windows(3).position(|window| {
        let search = &window[0]["Search"];
        let found = search
            .get("as")
            .cloned()
            .map(|bind| json!({ "Bound": bind }));
        search.get("also") == Some(&json!(["Graveyard"]))
            && search.get("zone").is_none()
            && search.get("who").is_none()
            && search.get("of").is_none()
            && found.is_some()
            && window[1]["Move"].get("what") == found.as_ref()
            && window[1]["Move"].get("to") == Some(&json!("Hand"))
            && window[2] == shuffle_if_searched()
    })?;
    let search = steps[at].get_mut("Search")?.as_object_mut()?;
    search.remove("also");
    // The zones searched are the searcher's own: "a card you own" says nothing more there.
    if let Some(filter) = search.get_mut("filter").and_then(Value::as_array_mut) {
        filter.retain(|pred| *pred != json!({ "OwnedBy": "You" }));
    }
    steps[at + 2] = json!({ "Shuffle": {} });
    Some(())
}

/// The ability with the search made plain, in its own steps or in a "you may".
fn plain(ability: &Ability) -> Option<Ability> {
    twin(ability, |value| {
        let body = super::twin_json::inner_mut(value)?;
        let steps = body.get_mut("effects")?.as_array_mut()?;
        if plain_steps(steps).is_some() {
            return Some(());
        }
        steps.iter_mut().find_map(|step| {
            let inner = step.get_mut("May")?;
            inner.get("who").is_none().then_some(())?;
            plain_steps(inner.get_mut("effects")?.as_array_mut()?)
        })
    })
}

/// Adds the wordings of a search of your library and/or graveyard that puts the card into your hand.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some(plain) = plain(ability) else {
        return;
    };
    for text in variants(&plain) {
        if text.matches("search your library for ").count() != 1 {
            continue;
        }
        let text = text.replacen(
            "search your library for ",
            "search your library and/or graveyard for ",
            1,
        );
        for pron in ["it", "them"] {
            let put = format!(", put {pron} into your hand, then shuffle");
            if text.matches(put.as_str()).count() != 1 {
                continue;
            }
            for join in ["and", "then"] {
                texts.push(text.replacen(
                    put.as_str(),
                    &format!(
                        ", {join} put {pron} into your hand. If you search your library this way, shuffle"
                    ),
                    1,
                ));
            }
        }
    }
}
