//! Counters on the creature of an event and on the card in one sentence: "put a +1/+1 counter on that creature and a
//! +1/+1 counter on this creature." The renderers write two sentences ("put a +1/+1 counter on it. Put a +1/+1 counter on
//! ~.").

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wording of counters put on the creature of the event and then on the card.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let effects = ability.effects();
    let pair = effects.windows(2).any(|pair| {
        matches!(
            pair,
            [
                Effect::PutCounters {
                    on: Ref::Triggering,
                    ..
                },
                Effect::PutCounters { on: Ref::This, .. }
            ]
        )
    });
    if !pair {
        return;
    }
    let said: Vec<String> = texts.iter().filter_map(|text| joined(text)).collect();
    texts.extend(said);
}

/// The text with "put A on it. (Then) put B on ~" as "put A on that creature and B on this creature".
fn joined(text: &str) -> Option<String> {
    let (head, rest) = text.split_once("put ")?;
    let (first, rest) = rest.split_once(" on it. ")?;
    let rest = rest
        .strip_prefix("Then put ")
        .or_else(|| rest.strip_prefix("Put "))?;
    let (second, tail) = rest.split_once(" on ~")?;
    Some(format!(
        "{head}put {first} on that creature and {second} on ~{tail}"
    ))
}
