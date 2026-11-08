//! Damage prevention keywords in words: "Prevent all [combat] damage that would be dealt to [and dealt by] CARDNAME." as
//! the replacements of the creature itself, or, given to other objects, as the events a shield or a static replacement
//! of the giver prevents.

use serde_json::{Value, json};

/// What a prevention keyword prevents: combat damage only or all of it, and damage dealt to and dealt by the object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prevention {
    /// Only combat damage (`Some(true)`); all damage when `None`.
    pub combat: Option<bool>,
    /// The damage dealt to the object.
    pub to: bool,
    /// The damage the object deals.
    pub by: bool,
}

/// The prevention a keyword word says, with or without its closing period; `None` for any other word.
pub fn prevention(word: &str) -> Option<Prevention> {
    let rest = word.trim().strip_prefix("Prevent all ")?;
    let (combat, rest) = match rest.strip_prefix("combat ") {
        Some(rest) => (Some(true), rest),
        None => (None, rest),
    };
    let rest = rest.strip_prefix("damage that would be dealt ")?;
    let (to, by) = match rest.trim_end_matches('.') {
        "to CARDNAME" => (true, false),
        "by CARDNAME" => (false, true),
        "to and dealt by CARDNAME" => (true, true),
        _ => return None,
    };
    Some(Prevention { combat, to, by })
}

impl Prevention {
    /// The damage events prevented, as `ReplaceWhen::Damage` values: to the recipient `to`, and from the sources `from`.
    pub fn events(self, to: &Value, from: &Value) -> Vec<Value> {
        let when = |mut damage: Value| {
            if let Some(combat) = self.combat {
                damage["combat"] = json!(combat);
            }
            json!({ "Damage": damage })
        };
        let mut all = Vec::new();
        if self.to {
            all.push(when(json!({ "to": to })));
        }
        if self.by {
            all.push(when(json!({ "source": from })));
        }
        all
    }

    /// The damage events prevented for the objects `affected` names: to them, and from them as they are now.
    pub fn events_of(self, affected: &Value) -> Vec<Value> {
        self.events(affected, &json!([{ "Is": affected }]))
    }
}

/// The replacements of a `K:` line of the form "Prevent all damage that would be dealt to CARDNAME."; `None` for any other.
pub fn prevent_keyword(body: &str) -> Option<Vec<Value>> {
    if !body.trim().ends_with('.') {
        return None;
    }
    let found = prevention(body)?;
    Some(
        found
            .events(&json!("This"), &json!(["This"]))
            .into_iter()
            .map(|when| json!({"Replacement": {"when": when, "by": "Prevent"}}))
            .collect(),
    )
}
