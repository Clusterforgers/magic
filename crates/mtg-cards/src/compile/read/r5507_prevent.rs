//! r55-07 prevention by counters (`ReplaceBy::PreventByCounters`): "For each 1 damage that would be dealt to ~, if it
//! has a +1/+1 counter on it, remove a +1/+1 counter from it and prevent that 1 damage.", read and written whole.

use super::counters::kind_text;
use crate::ability::Ability;
use crate::refs::{Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::CounterKind;

/// The sentence of a prevention by counters of the kind named by `kind`.
fn by_counters_text(kind: &str) -> String {
    let article = super::triggers::article(kind);
    format!(
        "For each 1 damage that would be dealt to ~, if it has {article} {kind} counter on it, remove {article} {kind} counter from it and prevent that 1 damage."
    )
}

/// The replacement of the sentence for counters of `counter`.
fn by_counters(counter: CounterKind) -> Ability {
    Ability::Replacement {
        when: ReplaceWhen::Damage {
            to: Some(Ref::This),
            source: Selector(Vec::new()),
            combat: None,
        },
        by: ReplaceBy::PreventByCounters(counter),
        condition: None,
        from: ZoneKind::Battlefield,
    }
}

/// The prevention by counters the sentence words.
pub(super) fn r5507_read(text: &str) -> Option<Ability> {
    let rest = text.strip_prefix("For each 1 damage that would be dealt to ")?;
    let rest = ["~", "this creature", "this permanent"]
        .iter()
        .find_map(|name| rest.strip_prefix(name))?;
    let rest = rest.strip_prefix(", if it has ")?;
    let (_, rest) = rest.split_once(' ')?;
    let (kind, _) = rest.split_once(" counter on it, ")?;
    let counter: CounterKind =
        serde_json::from_value(serde_json::Value::String(kind.to_owned())).ok()?;
    let written = by_counters_text(kind_text(&counter)?);
    let tail = |sentence: &str| {
        sentence
            .split_once(", if it has ")
            .map(|(_, tail)| tail.to_owned())
    };
    (tail(&written)? == tail(text)?).then(|| by_counters(counter))
}

/// The sentence of a prevention by counters.
pub(super) fn r5507_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        when:
            ReplaceWhen::Damage {
                to: Some(Ref::This),
                source,
                combat: None,
            },
        by: ReplaceBy::PreventByCounters(counter),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return;
    };
    if let Some(kind) = kind_text(counter).filter(|_| source.0.is_empty()) {
        let text = by_counters_text(kind);
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
