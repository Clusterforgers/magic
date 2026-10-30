//! A spell whose target depends on the gift: "Counter target creature spell. If the gift was promised, instead counter
//! target spell." The spell has two target specifications and the promised gift selects the second in place of the first.

use super::{Effect, Ref, TargetSpec, spec_text};
use crate::refs::Bounds;
use crate::value::Condition;

/// The count of the first target: one unless the gift was promised.
const UNPROMISED: &str = r#"{"min":1,"max":{"Sum":[1,{"Times":{"by":-1,"of":"Promised"}}]}}"#;
/// The count of the second target: one when the gift was promised.
const PROMISED: &str = r#"{"min":1,"max":"Promised"}"#;

/// The bounds a table entry names.
fn bounds(json: &str) -> Option<Bounds> {
    serde_json::from_str(json).ok()
}

/// The target as a sentence says it, with its count left out.
fn named(spec: &TargetSpec) -> Option<String> {
    spec_text(&TargetSpec {
        bounds: Bounds::default(),
        ..spec.clone()
    })
}

/// The sentences of a counter that another target replaces when the gift was promised.
pub(super) fn sentences(targets: &[TargetSpec], effects: &[Effect]) -> Option<Vec<String>> {
    let [first, second] = targets else {
        return None;
    };
    let [
        Effect::If {
            condition: Condition::AdditionalCostPaid,
            then,
            otherwise,
        },
    ] = effects
    else {
        return None;
    };
    let [
        Effect::Counter {
            what: Ref::Target(1),
        },
    ] = then.as_slice()
    else {
        return None;
    };
    let [
        Effect::Counter {
            what: Ref::Target(0),
        },
    ] = otherwise.as_slice()
    else {
        return None;
    };
    (Some(&first.bounds) == bounds(UNPROMISED).as_ref()
        && Some(&second.bounds) == bounds(PROMISED).as_ref())
    .then_some(())?;
    Some(vec![
        format!("counter {}", named(first)?),
        format!(
            "if the gift was promised, instead counter {}",
            named(second)?
        ),
    ])
}
