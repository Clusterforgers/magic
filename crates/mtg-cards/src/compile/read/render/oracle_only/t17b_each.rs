//! Several targets that each change: "Up to two target creatures each get +2/+2 and gain trample until end of turn."
//! The renderers say "Up to two target creatures get ..."; "each" follows the targets before the verb.

use super::super::super::selectors::spec_text;
use super::super::variants;
use super::w4b_stand::json;
use crate::ability::Ability;
use crate::refs::TargetSpec;
use serde_json::{Value, json};

/// The verbs a change of the targets opens with.
const VERBS: [&str; 5] = ["get", "gain", "lose", "have", "become"];

/// Adds the wordings of an ability whose continuous effect changes several targets, each of them.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some(spec) = several_targets(&json(ability)) else {
        return;
    };
    let Some(words) = spec_text(&spec) else {
        return;
    };
    let capital = super::super::capitalize(&words);
    for text in variants(ability) {
        for (subject, with_each) in [
            (&words, format!("{words} each")),
            (&capital, format!("{capital} each")),
        ] {
            for verb in VERBS {
                let from = format!("{subject} {verb} ");
                if text.contains(&from) {
                    texts.push(text.replacen(&from, &format!("{with_each} {verb} "), 1));
                }
            }
        }
    }
}

/// The first target of an ability that changes it with a continuous effect, when it takes more than one.
fn several_targets(value: &Value) -> Option<TargetSpec> {
    let body = value.as_object()?.values().next()?;
    let changes_target = body
        .get("effects")?
        .as_array()?
        .iter()
        .any(|effect| effect.pointer("/Continuous/affected") == Some(&json!({"Target": 0})));
    changes_target.then_some(())?;
    let spec: TargetSpec = serde_json::from_value(body.pointer("/targets/0")?.clone()).ok()?;
    (!spec.bounds.is_one()).then_some(spec)
}
