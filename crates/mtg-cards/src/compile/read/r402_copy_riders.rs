//! r40-2 riders of a spell copy and their writers: "If it's a permanent spell, the copy gains haste and "At the beginning
//! of the end step, sacrifice this permanent."" after a copy of a spell is the copy's `except` (a copy of a permanent
//! spell becomes a token with them, CR 707.10f; an instant or sorcery copy is unchanged by either).

use serde_json::{Value, json};

use crate::ability::Ability;

/// The rider sentences and whose end step each sacrifice waits for (`None` for each end step).
const RIDERS: [(&str, Option<&str>); 2] = [
    (
        " If it's a permanent spell, the copy gains haste and \"At the beginning of the end step, sacrifice this permanent.\"",
        None,
    ),
    (
        " If the copy is a permanent spell, it gains haste and \"At the beginning of your end step, sacrifice this permanent.\"",
        Some("You"),
    ),
];

/// The changes a rider gives the copy: haste and the sacrifice at the end step.
fn changes(whose: Option<&str>) -> Value {
    let mut when = json!({ "StepBegins": { "step": "End" } });
    if let Some(whose) = whose {
        when["StepBegins"]["whose"] = json!(whose);
    }
    json!([
        { "AddKeyword": "Haste" },
        { "GrantAbility": { "Triggered": {
            "when": when,
            "effects": [{ "Sacrifice": { "filter": ["This"] } }]
        } } }
    ])
}

/// The one `CopySpell` step in the ability's JSON, wherever it is nested; `None` when there is not exactly one.
fn copy_step(value: &mut Value) -> Option<&mut Value> {
    (copies(value) == 1).then_some(())?;
    first_copy(value)
}

/// How many `CopySpell` steps the tree holds.
fn copies(value: &Value) -> usize {
    match value {
        Value::Object(map) if map.contains_key("CopySpell") => 1,
        Value::Object(map) => map.values().map(copies).sum(),
        Value::Array(items) => items.iter().map(copies).sum(),
        _ => 0,
    }
}

/// The body of the first `CopySpell` step in the tree.
fn first_copy(value: &mut Value) -> Option<&mut Value> {
    match value {
        Value::Object(map) => {
            if map.contains_key("CopySpell") {
                return map.get_mut("CopySpell");
            }
            map.values_mut().find_map(first_copy)
        }
        Value::Array(items) => items.iter_mut().find_map(first_copy),
        _ => None,
    }
}

/// The clause read without its rider, then the rider's changes given to its copy; `None` when the clause ends with no
/// rider or its head copies no spell.
pub(super) fn r402_rider_read(
    text: &str,
    head: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    let (rest, whose) = RIDERS
        .iter()
        .find_map(|(rider, whose)| text.strip_suffix(rider).map(|rest| (rest, *whose)))?;
    let read = head(rest)?;
    let mut value = serde_json::to_value(&read).ok()?;
    let body = copy_step(&mut value)?;
    let has_except = body
        .get("except")
        .is_some_and(|except| except != &json!([]));
    if has_except {
        return None;
    }
    body.as_object_mut()?
        .insert("except".to_owned(), changes(whose));
    serde_json::from_value(value).ok()
}

/// The wordings of an ability whose copy of a spell carries a rider: the wordings of the ability without it, each followed
/// by the rider; `None` for any other ability.
pub(super) fn r402_rider_variants(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    if !matches!(ability, Ability::Triggered { .. }) {
        return None;
    }
    let mut value = serde_json::to_value(ability).ok()?;
    let body = copy_step(&mut value)?;
    let except = body.get("except")?.clone();
    let (rider, _) = RIDERS.iter().find(|(_, whose)| changes(*whose) == except)?;
    body.as_object_mut()?.remove("except");
    let twin: Ability = serde_json::from_value(value).ok()?;
    Some(
        written(&twin)
            .into_iter()
            .map(|text| format!("{text}{rider}"))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    //! Goal: a copy's rider reads into the copy's changes and back, whose end step it names stays apart, and a rider on a
    //! clause that copies no spell, or another rider, reads nothing.
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use rstest::rstest;

    use super::*;

    /// A trigger that copies the spell cast, with `except` when given.
    fn copying(except: Option<Value>) -> Ability {
        let mut body = json!({ "what": "Triggering", "new_targets": true });
        if let Some(except) = except {
            body["except"] = except;
        }
        serde_json::from_value(json!({ "Triggered": {
            "when": { "SpellCast": { "what": [{ "CastFrom": "Exile" }], "by": "You" } },
            "effects": [{ "CopySpell": body }]
        } }))
        .unwrap()
    }

    const HEAD: &str = "Whenever you cast a spell from exile, copy it.";

    #[rstest]
    #[case::each_end_step(0, Some(changes(None)))]
    #[case::your_end_step(1, Some(changes(Some("You"))))]
    fn r402_a_rider_reads_into_the_copy(#[case] rider: usize, #[case] expected: Option<Value>) {
        let text = format!("{HEAD}{}", RIDERS[rider].0);
        let found = r402_rider_read(&text, |head| (head == HEAD).then(|| copying(None)));
        assert_eq!(found, Some(copying(expected)));
        let texts = r402_rider_variants(&found.unwrap(), |_| vec![HEAD.to_owned()]).unwrap();
        assert_eq!(texts, vec![text]);
    }

    #[rstest]
    #[case::no_rider(HEAD.to_owned())]
    #[case::another_rider(format!("{HEAD} If it's a permanent spell, the copy gains flying."))]
    fn r402_no_rider_reads_nothing(#[case] text: String) {
        assert_eq!(r402_rider_read(&text, |_| Some(copying(None))), None);
    }

    #[test]
    fn r402_a_head_that_copies_no_spell_reads_nothing() {
        let text = format!("{HEAD}{}", RIDERS[0].0);
        let drawing: Ability =
            serde_json::from_value(json!({ "Spell": { "effects": [{ "Draw": {} }] } })).unwrap();
        assert_eq!(r402_rider_read(&text, |_| Some(drawing.clone())), None);
        assert_eq!(r402_rider_variants(&drawing, |_| Vec::new()), None);
    }
}
