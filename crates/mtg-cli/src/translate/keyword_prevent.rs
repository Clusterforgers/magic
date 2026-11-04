//! Damage prevention keywords in words: "Prevent all [combat] damage that would be dealt to [and dealt by] CARDNAME." as
//! the replacements of the creature itself.

use serde_json::{Value, json};

/// The replacements of a `K:` line of the form "Prevent all damage that would be dealt to CARDNAME."; `None` for any other.
pub fn prevent_keyword(body: &str) -> Option<Vec<Value>> {
    let rest = body.trim().strip_prefix("Prevent all ")?;
    let (combat, rest) = match rest.strip_prefix("combat ") {
        Some(rest) => (Some(true), rest),
        None => (None, rest),
    };
    let rest = rest.strip_prefix("damage that would be dealt ")?;
    let (to, by) = match rest {
        "to CARDNAME." => (true, false),
        "by CARDNAME." => (false, true),
        "to and dealt by CARDNAME." => (true, true),
        _ => return None,
    };
    let when = |damage: Value| {
        let mut damage = damage;
        if let Some(combat) = combat {
            damage["combat"] = json!(combat);
        }
        json!({"Replacement": {"when": {"Damage": damage}, "by": "Prevent"}})
    };
    let mut all = Vec::new();
    if to {
        all.push(when(json!({"to": "This"})));
    }
    if by {
        all.push(when(json!({"source": ["This"]})));
    }
    Some(all)
}
