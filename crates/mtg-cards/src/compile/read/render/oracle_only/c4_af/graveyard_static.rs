//! A static ability or a replacement that works while the card is in its owner's graveyard: "As long as this card is in
//! your graveyard and you control a Forest, creatures you control have trample." The wording of the same ability on the
//! battlefield gets the condition.

use super::super::twin_json::{lowered, twin};
use crate::ability::Ability;
use crate::refs::ZoneKind;

/// The group as the renderers write it and as the card writes it.
const GROUPS: [(&str, &str); 2] = [
    (
        "each creature you control has",
        "creatures you control have",
    ),
    (
        "each creature you control gets",
        "creatures you control get",
    ),
];

/// Adds the wordings of an ability that works from the graveyard.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let from_graveyard = matches!(
        ability,
        Ability::Static {
            from: ZoneKind::Graveyard,
            ..
        } | Ability::Replacement {
            from: ZoneKind::Graveyard,
            ..
        }
    );
    if !from_graveyard {
        return;
    }
    let Some(plain) = twin(ability, |value| {
        let key = ["Static", "Replacement"]
            .into_iter()
            .find(|key| value.get(key).is_some())?;
        value
            .get_mut(key)?
            .as_object_mut()?
            .remove("from")
            .map(|_| ())
    }) else {
        return;
    };
    let said: Vec<String> = super::super::super::variants(&plain)
        .iter()
        .flat_map(|text| {
            let swapped = GROUPS
                .iter()
                .fold(text.clone(), |text, (from, to)| text.replacen(from, to, 1));
            [text.clone(), swapped]
        })
        .map(|text| graveyard(&text))
        .collect();
    texts.extend(said);
}

/// The text with the card in its graveyard as the first condition.
fn graveyard(text: &str) -> String {
    match text
        .strip_prefix("As long as ")
        .and_then(|rest| rest.split_once(", "))
    {
        Some((condition, body)) => {
            format!("As long as ~ is in your graveyard and {condition}, {body}")
        }
        None => format!("As long as ~ is in your graveyard, {}", lowered(text)),
    }
}
