//! "That creature" in a static ability over a group is each object the static affects ("Each nontoken creature you
//! control gets +1/+1 for each other creature you control with the same name as that creature"): a static has no
//! targets, so the reader's `Target(0)` is `Affected`, and "other" in a count that names it is other than that object.

use serde_json::{Value, json};

use crate::ability::Ability;
use crate::refs::Ref;

/// The group static with "that creature" as the affected object and "other" as other than it; any other ability as is.
pub(super) fn r117_04_affected(ability: Ability) -> Ability {
    let Ability::Static {
        affected: Ref::Each(_),
        ..
    } = &ability
    else {
        return ability;
    };
    let Ok(mut value) = serde_json::to_value(&ability) else {
        return ability;
    };
    let Some(mods) = value.pointer_mut("/Static/mods") else {
        return ability;
    };
    if grants(mods) || !named(mods, &json!({"Target": 0})) {
        return ability;
    }
    swap(mods, &json!({"Target": 0}), &json!("Affected"));
    others(mods, &json!("Other"), &json!({"Not": {"Is": "Affected"}}));
    serde_json::from_value(value).unwrap_or(ability)
}

/// The wordings of a group static whose counts name the affected object: those of the same static as the reader
/// reads it from "that creature" and "other".
pub(super) fn r117_04_affected_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ability::Static {
        affected: Ref::Each(_),
        ..
    } = ability
    else {
        return;
    };
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(mods) = value.pointer_mut("/Static/mods") else {
        return;
    };
    if grants(mods) || !named(mods, &json!("Affected")) {
        return;
    }
    others(mods, &json!({"Not": {"Is": "Affected"}}), &json!("Other"));
    swap(mods, &json!("Affected"), &json!({"Target": 0}));
    let Ok(read) = serde_json::from_value::<Ability>(value) else {
        return;
    };
    // Only a static the reader makes again of the same words.
    if r117_04_affected(read.clone()) != *ability {
        return;
    }
    for text in written(&read) {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// Whether a modification grants an ability, whose own words name its own objects ("target player").
fn grants(mods: &Value) -> bool {
    mods.as_array()
        .is_some_and(|mods| mods.iter().any(|m| m.get("GrantAbility").is_some()))
}

/// Whether the value holds `what` anywhere.
fn named(value: &Value, what: &Value) -> bool {
    value == what
        || match value {
            Value::Array(items) => items.iter().any(|item| named(item, what)),
            Value::Object(map) => map.values().any(|item| named(item, what)),
            _ => false,
        }
}

/// Replaces every `from` with `to`.
fn swap(value: &mut Value, from: &Value, to: &Value) {
    if value == from {
        *value = to.clone();
        return;
    }
    match value {
        Value::Array(items) => items.iter_mut().for_each(|item| swap(item, from, to)),
        Value::Object(map) => map.values_mut().for_each(|item| swap(item, from, to)),
        _ => {}
    }
}

/// Replaces `from` with `to` in each selector that names the affected object elsewhere.
fn others(value: &mut Value, from: &Value, to: &Value) {
    match value {
        Value::Array(items) => {
            let about_it = items
                .iter()
                .any(|item| item != from && item != to && named(item, &json!("Affected")));
            for item in items.iter_mut() {
                if about_it && item == from {
                    *item = to.clone();
                } else {
                    others(item, from, to);
                }
            }
        }
        Value::Object(map) => map.values_mut().for_each(|item| others(item, from, to)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    //! Goal: "that creature" and "other" in a count of a group static read as the affected object and other than it and
    //! write back as the same text; a static of the card itself, or a count that does not name the object, is unchanged.
    #![allow(clippy::unwrap_used)]
    use crate::ability::Ability;
    use crate::compile::ClauseKind;
    use crate::compile::read::clause;
    use crate::compile::read::render::written_variants;
    use rstest::rstest;

    const MIRROR: &str = "Each nontoken creature you control gets +1/+1 for each other creature you control with the same name as that creature.";
    const MIRROR_JSON: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":{"Count":[{"Type":"Creature"},{"Not":{"Is":"Affected"}},{"ControlledBy":"You"},{"SharesName":"Affected"}]},"toughness":{"Count":[{"Type":"Creature"},{"Not":{"Is":"Affected"}},{"ControlledBy":"You"},{"SharesName":"Affected"}]}}}]}}"#;

    #[test]
    fn r117_04_that_creature_is_the_affected_object() {
        let read = clause(MIRROR, ClauseKind::Static, false).unwrap();
        let expected: Ability = serde_json::from_str(MIRROR_JSON).unwrap();
        assert_eq!(read, expected);
        let texts = written_variants(&read);
        assert!(texts.iter().any(|t| t == MIRROR), "{texts:#?}");
    }

    #[rstest]
    #[case::the_source_counted(&MIRROR_JSON.replace(r#"{"Not":{"Is":"Affected"}}"#, r#""Other""#))]
    #[case::the_card_itself(&MIRROR_JSON.replace(r#""affected":{"Each":[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"You"}]}"#, r#""affected":"This""#))]
    fn r117_04_another_static_is_not_the_text(#[case] json: &str) {
        let ability: Ability = serde_json::from_str(json).unwrap();
        let texts = written_variants(&ability);
        assert!(!texts.iter().any(|t| t == MIRROR), "{texts:#?}");
    }
}
