//! Trigger lines that name who caused the event (`ValidCause$`, and `ValidSource$` of a counter put): the event pattern
//! wrapped in `Caused`, by the controllers and sources the word names.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use crate::forge::script::Params;

/// The modes whose events a spell or ability can cause, with the key that names the cause.
const MODES: [(&str, &str); 7] = [
    ("CounterAdded", "ValidSource"),
    ("CounterAddedOnce", "ValidSource"),
    ("Discarded", "ValidCause"),
    ("DiscardedAll", "ValidCause"),
    ("ChangesZone", "ValidCause"),
    ("ChangesZoneAll", "ValidCause"),
    ("TurnFaceUp", "ValidCause"),
];

/// The pattern of a line with a cause word, `None` for a line without one.
pub fn pattern(ctx: &Ctx, p: &Params, mode: &str) -> Option<Tr<Value>> {
    let (_, key) = MODES.iter().find(|(known, _)| *known == mode)?;
    let word = p.get(key)?;
    let by = match players(word) {
        Some(by) => by,
        None => return Some(unmapped(format!("key:Trigger.{mode}.{key}={word}"))),
    };
    let inner = match super::triggers::pattern(ctx, &p.without(&[key])) {
        Ok(inner) => inner,
        Err(gap) => return Some(Err(gap)),
    };
    let mut caused = json!({ "inner": inner });
    if let Some(by) = by {
        caused["by"] = json!(by);
    }
    Some(Ok(json!({ "Caused": caused })))
}

/// The players a cause word names: `Some(None)` for any, `None` for a word that names the cause some other way. The spell or
/// ability is the cause whatever the word says: no turn action and no state-based action ever matches.
fn players(word: &str) -> Option<Option<&'static str>> {
    match word {
        "SpellAbility" => Some(None),
        "You" | "SpellAbility.YouCtrl" => Some(Some("You")),
        "Opponent" | "Player.Opponent" | "SpellAbility.OppCtrl" => Some(Some("Opponents")),
        _ => None,
    }
}
