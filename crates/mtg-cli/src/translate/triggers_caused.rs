//! Trigger lines that name who caused the event (`ValidCause$`, and `ValidSource$` of a counter put): the event pattern
//! wrapped in `Caused`, by the controllers and sources the word names.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use crate::forge::script::Params;

/// The modes whose events a spell or ability can cause, with the key that names the cause.
const MODES: [(&str, &str); 9] = [
    ("Taps", "ValidPlayer"),
    ("CounterPlayerAddedAll", "ValidSource"),
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
    let written = p.get(key);
    let word = written.or_else(|| you_put(p, mode))?;
    // The words stand for the cause only once: the inner pattern must not read them again.
    let key: &str = if written.is_some() {
        key
    } else {
        "TriggerDescription"
    };
    let Ok(by) = players(word) else {
        return Some(unmapped(format!("key:Trigger.{mode}.{key}={word}")));
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

/// "Whenever you put one or more time counters": the script names no cause (`CounterAddedOnce` without `ValidSource$`), the
/// printed words do, and counters an opponent puts do not count (Kate Stewart).
fn you_put<'a>(p: &'a Params, mode: &str) -> Option<&'a str> {
    let counters = matches!(
        mode,
        "CounterAdded" | "CounterAddedOnce" | "CounterPlayerAddedAll"
    );
    let text = p.get("TriggerDescription")?.to_lowercase();
    (counters && text.contains("whenever you put ")).then_some("You")
}

/// The players a cause word names: `Ok(None)` for any, `Err` for a word that names the cause some other way. The spell or
/// ability is the cause whatever the word says: no turn action and no state-based action ever matches.
fn players(word: &str) -> Result<Option<&'static str>, ()> {
    match word {
        "SpellAbility" => Ok(None),
        "You" | "SpellAbility.YouCtrl" => Ok(Some("You")),
        "Opponent" | "Player.Opponent" | "SpellAbility.OppCtrl" => Ok(Some("Opponents")),
        _ => Err(()),
    }
}
