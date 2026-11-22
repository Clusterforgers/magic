//! A token the text names alone and its reminder text defines ("Create a Heartwood token. (It's a red and green artifact
//! with "{T}: Add {R} or {G}.")"): a custom token that is exactly that definition is also worded by its name.

use std::sync::OnceLock;

use serde_json::Value;

use crate::ability::Ability;

/// The tokens the reminder text defines: the name the text uses and the token as data.
const DEFINED: &[(&str, &str)] = &[
    (
        "Heartwood",
        r#"{"name":"Heartwood","colors":["Red","Green"],"types":["Artifact"],"subtypes":["Heartwood"],"abilities":[{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["Red","Green"]}}}]}}]}"#,
    ),
    (
        "Shard",
        r#"{"name":"Shard","types":["Enchantment"],"subtypes":["Shard"],"abilities":[{"Activated":{"cost":[{"Mana":"{2}"},"SacrificeSelf"],"effects":[{"Scry":{}},{"Draw":{}}]}}]}"#,
    ),
];

/// The token with its colors in one order: a token has no order of colors.
fn normal(mut token: Value) -> Value {
    if let Some(Value::Array(colors)) = token.get_mut("colors") {
        colors.sort_by_key(Value::to_string);
    }
    token
}

/// The defined tokens, parsed once.
fn defined() -> &'static [(&'static str, Value)] {
    static PARSED: OnceLock<Vec<(&'static str, Value)>> = OnceLock::new();
    PARSED.get_or_init(|| {
        DEFINED
            .iter()
            .filter_map(|(name, json)| Some((*name, normal(serde_json::from_str(json).ok()?))))
            .collect()
    })
}

/// The name of the defined token that a custom token is exactly.
fn named(custom: &Value) -> Option<&'static str> {
    let custom = normal(custom.clone());
    defined()
        .iter()
        .find_map(|(name, token)| (*token == custom).then_some(*name))
}

/// Replaces every custom token that is a defined one with the token of that card name, which is worded by its name
/// alone; true when one was replaced.
fn rename(value: &mut Value) -> bool {
    match value {
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |found, item| rename(item) | found),
        Value::Object(map) => {
            let hit = map.get("Custom").filter(|_| map.len() == 1).and_then(named);
            if let Some(name) = hit {
                *value = serde_json::json!({ "Card": name });
                return true;
            }
            map.values_mut()
                .fold(false, |found, item| rename(item) | found)
        }
        _ => false,
    }
}

/// Adds the wordings of an ability whose custom tokens that the text names alone are worded by their names.
pub(super) fn r116_reminder_token_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ok(mut data) = serde_json::to_value(ability) else {
        return;
    };
    if !rename(&mut data) {
        return;
    }
    // The twin only gives the wording: a token of a card's name is worded by that name.
    let Ok(twin) = serde_json::from_value::<Ability>(data) else {
        return;
    };
    for text in written(&twin) {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a custom token that is exactly what a reminder text defines is also worded by its name (alone, counted,
    //! tapped, inside a payment), and a token that differs in a color, a type, a name or an ability is not.
    #![allow(clippy::unwrap_used)]
    use super::{DEFINED, defined};
    use crate::ability::Ability;
    use crate::card::TokenSpec;
    use crate::compile::read::render::written_variants;
    use rstest::rstest;

    fn heartwood(token: &str) -> String {
        format!(
            r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"CreateToken":{{"token":{{"Custom":{token}}}}}}}]}}}}"#
        )
    }

    const HEARTWOOD: &str = DEFINED[0].1;

    #[test]
    fn every_defined_token_is_a_token() {
        assert_eq!(defined().len(), DEFINED.len());
        for (_, json) in DEFINED {
            serde_json::from_str::<TokenSpec>(json).unwrap();
        }
    }

    #[rstest]
    #[case::alone(heartwood(HEARTWOOD), "When ~ enters, create a Heartwood token.")]
    #[case::colors_in_the_other_order(
        heartwood(&HEARTWOOD.replace(r#"["Red","Green"],"types""#, r#"["Green","Red"],"types""#)),
        "When ~ enters, create a Heartwood token."
    )]
    #[case::counted(
        heartwood(HEARTWOOD).replace(r"}}}}]}}", r#"}},"count":2,"tapped":true}}]}}"#),
        "When ~ enters, create two tapped Heartwood tokens."
    )]
    #[case::shard(
        heartwood(DEFINED[1].1).replace(r"}}}}]}}", r#"}},"count":"X"}}]}}"#),
        "When ~ enters, create X Shard tokens."
    )]
    #[case::paid(
        format!(
            r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"Pay":{{"cost":[{{"Sacrifice":{{"filter":[{{"Type":"Land"}}]}}}}],"paid":[{{"CreateToken":{{"token":{{"Custom":{HEARTWOOD}}}}}}}]}}}}]}}}}"#
        ),
        "When ~ enters, you may sacrifice a land, then if you do, you create a Heartwood token."
    )]
    fn a_defined_token_is_worded_by_its_name(#[case] json: String, #[case] text: &str) {
        let ability: Ability = serde_json::from_str(&json).unwrap();
        let texts = written_variants(&ability);
        assert!(texts.iter().any(|t| t == text), "{texts:#?}");
    }

    #[rstest]
    #[case::another_color(HEARTWOOD.replace(r#""Red","Green"],"types""#, r#""Red"],"types""#))]
    #[case::another_type(HEARTWOOD.replace(r#""types":["Artifact"]"#, r#""types":["Enchantment"]"#))]
    #[case::another_name(HEARTWOOD.replace(r#""name":"Heartwood""#, r#""name":"Heart""#))]
    #[case::another_ability(HEARTWOOD.replace(r#""cost":["Tap"]"#, r#""cost":["Tap","SacrificeSelf"]"#))]
    #[case::no_ability(r#"{"name":"Heartwood","colors":["Red","Green"],"types":["Artifact"],"subtypes":["Heartwood"]}"#.to_owned())]
    fn a_token_that_differs_from_its_definition_is_not_worded_by_its_name(#[case] token: String) {
        let ability: Ability = serde_json::from_str(&heartwood(&token)).unwrap();
        let texts = written_variants(&ability);
        assert!(
            !texts.iter().any(|t| t.contains("a Heartwood token")),
            "{texts:#?}"
        );
    }
}
