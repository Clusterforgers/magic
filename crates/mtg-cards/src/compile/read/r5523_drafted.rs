//! r55-23 wording of mana of the colors a booster draft chose: "Add one mana of any color chosen as you drafted cards
//! named ~" (`Produce::DraftedColors`). It is written and read through a twin that adds one mana of any color, which the
//! other writers and readers word.

use serde_json::{Value, json};

use crate::ability::Ability;

/// The words of the drafted colors after "Add".
const R5523_DRAFTED_WORDS: &str = "one mana of any color chosen as you drafted cards named ~";
/// The twin's words after "Add".
const R5523_TWIN_WORDS: &str = "one mana of any color";

/// Replaces every `from` value of an `AddMana`'s `mana` by `to` and says how many it replaced.
fn r5523_swap(value: &mut Value, from: &Value, to: &Value) -> usize {
    match value {
        Value::Object(map) => {
            let mut swapped = 0;
            if let Some(Value::Object(add)) = map.get_mut("AddMana")
                && let Some(mana) = add.get_mut("mana")
                && mana == from
            {
                *mana = to.clone();
                swapped += 1;
            }
            swapped
                + map
                    .values_mut()
                    .map(|inner| r5523_swap(inner, from, to))
                    .sum::<usize>()
        }
        Value::Array(items) => items
            .iter_mut()
            .map(|inner| r5523_swap(inner, from, to))
            .sum(),
        _ => 0,
    }
}

/// The ability with its one `mana` swapped from `from` to `to`; `None` unless exactly one is found.
fn r5523_swapped(ability: &Ability, from: &Value, to: &Value) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    (r5523_swap(&mut value, from, to) == 1).then_some(())?;
    serde_json::from_value(value).ok()
}

/// The JSON of the drafted colors.
fn r5523_drafted() -> Value {
    json!("DraftedColors")
}

/// The JSON of the twin's mana.
fn r5523_twin() -> Value {
    json!({ "AnyColor": 1 })
}

/// The wordings of an ability that adds mana of the drafted colors: the twin's wordings with the drafted words.
pub(super) fn r5523_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some(twin) = r5523_swapped(ability, &r5523_drafted(), &r5523_twin()) else {
        return;
    };
    let found: Vec<String> = written(&twin)
        .iter()
        .filter(|text| text.matches(R5523_TWIN_WORDS).count() == 1)
        .map(|text| text.replacen(R5523_TWIN_WORDS, R5523_DRAFTED_WORDS, 1))
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// Reads a clause that adds mana of the drafted colors: the twin's text read by `read`, its mana swapped back.
pub(super) fn r5523_read(text: &str, read: impl Fn(&str) -> Option<Ability>) -> Option<Ability> {
    (text.matches(R5523_DRAFTED_WORDS).count() == 1).then_some(())?;
    let twin = read(&text.replacen(R5523_DRAFTED_WORDS, R5523_TWIN_WORDS, 1))?;
    r5523_swapped(&twin, &r5523_twin(), &r5523_drafted())
}

#[cfg(test)]
mod tests {
    //! Goal: the drafted colors read and write through their twin, and only an ability with one such mana has a twin.
    use super::*;
    use rstest::rstest;

    fn ability(json: &str) -> Ability {
        serde_json::from_str(json).unwrap_or_else(|error| panic!("{json}: {error}"))
    }

    #[rstest]
    #[case::drafted(
        r#"{"Activated":{"cost":"{T}","effects":[{"AddMana":{"mana":"DraftedColors"}}]}}"#,
        true
    )]
    #[case::any_color(
        r#"{"Activated":{"cost":"{T}","effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        false
    )]
    #[case::twice(
        r#"{"Activated":{"cost":"{T}","effects":[{"AddMana":{"mana":"DraftedColors"}},{"AddMana":{"mana":"DraftedColors"}}]}}"#,
        false
    )]
    fn r5523_only_one_drafted_mana_has_a_twin(#[case] json: &str, #[case] twinned: bool) {
        let found = r5523_swapped(&ability(json), &r5523_drafted(), &r5523_twin());
        assert_eq!(found.is_some(), twinned);
    }

    #[rstest]
    #[case::drafted(
        "{T}: Add one mana of any color chosen as you drafted cards named ~.",
        true
    )]
    #[case::any_color("{T}: Add one mana of any color.", false)]
    #[case::another_note(
        "{T}: Add one mana of any color chosen as you drafted cards named Paliano.",
        false
    )]
    fn r5523_only_the_drafted_words_read(#[case] text: &str, #[case] read: bool) {
        let found = r5523_read(text, |twin| {
            (twin == "{T}: Add one mana of any color.")
                .then(|| ability(r#"{"Activated":{"cost":"{T}","effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#))
        });
        assert_eq!(found.is_some(), read, "{text}");
        if let Some(found) = found {
            assert!(
                serde_json::to_string(&found)
                    .unwrap_or_default()
                    .contains("DraftedColors")
            );
        }
    }
}
