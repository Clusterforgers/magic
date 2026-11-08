//! Cards exiled from the top of the library by a count of counters on the card: "exile the top X cards of your library,
//! where X is the number of skewer counters on ~. You may play those cards this turn." The ability is worded with a
//! fixed count and the count is said afterwards, with the permission as a sentence of its own.

use serde_json::{Value, json};

use super::super::written_variants;
use crate::ability::Ability;
use crate::value::CounterKind;

/// The count the twin exiles, which the wording of the count replaces.
const STAND_IN: &str = "exile the top seven cards of your library";

/// How the renderers join the permission to the exile, and the sentence the card prints instead.
const PERMISSIONS: [(&str, &str); 5] = [
    (" and may play ", ". You may play "),
    (" and you may play ", ". You may play "),
    (
        " and until end of turn, you may play",
        ". Until end of turn, you may play",
    ),
    (
        ", then until end of turn, you may play",
        ". Until end of turn, you may play",
    ),
    (", then you may play ", ". You may play "),
];

/// Adds the wordings of an ability that exiles as many cards as there are counters of a kind on the card.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(counter) = swap_count(&mut value) else {
        return;
    };
    let Some(kind) = super::super::super::counters::kind_text(&counter) else {
        return;
    };
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return;
    };
    for text in written_variants(&twin) {
        let Some((head, tail)) = text.split_once(STAND_IN) else {
            continue;
        };
        let count = format!(
            "exile the top X cards of your library, where X is the number of {kind} counters on ~"
        );
        let tail = PERMISSIONS
            .iter()
            .find_map(|(said, wanted)| {
                tail.strip_prefix(said)
                    .map(|rest| format!("{wanted}{rest}"))
            })
            .unwrap_or_else(|| tail.to_owned());
        let text = format!("{head}{count}{tail}");
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// Replaces the count of the first exile from the top that counts counters on the card by seven; the kind of counter.
fn swap_count(value: &mut Value) -> Option<CounterKind> {
    match value {
        Value::Object(map) => {
            if let Some(body) = map.get_mut("ExileTop")
                && let Some(counted) = body.get("count").and_then(|c| c.get("Counters"))
                && counted.get("on") == Some(&json!("This"))
                && let Ok(counter) = serde_json::from_value(counted.get("counter")?.clone())
            {
                body["count"] = json!(7);
                return Some(counter);
            }
            map.values_mut().find_map(swap_count)
        }
        Value::Array(list) => list.iter_mut().find_map(swap_count),
        _ => None,
    }
}
