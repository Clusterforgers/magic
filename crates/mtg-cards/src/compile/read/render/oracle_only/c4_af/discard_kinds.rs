//! A discard of one kind of card as an event: "Whenever you discard a creature card, create a 2/2 black Zombie creature
//! token." The renderers write the event for a card and put "you" before the verb of a token.

use super::super::twin_json::twin;
use crate::ability::Ability;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;
use serde_json::Value;

/// The selectors of the cards discarded and the card as the sentence names it.
const KINDS: [(&str, &str); 3] = [
    (r#"[{"Type":"Creature"}]"#, "a creature card"),
    (r#"[{"Type":"Land"}]"#, "a land card"),
    (
        r#"[{"Not":{"Type":"Creature"}},{"Not":{"Type":"Land"}}]"#,
        "a noncreature, nonland card",
    ),
];

/// Adds the wordings of a trigger on the discard of cards of one kind.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::Discarded { what, by: Ref::You },
        ..
    } = ability
    else {
        return;
    };
    let Ok(selector) = serde_json::to_string(what) else {
        return;
    };
    let Some((_, card)) = KINDS.iter().find(|(key, _)| *key == selector) else {
        return;
    };
    let Some(plain) = twin(ability, |value| {
        let discarded = value
            .get_mut("Triggered")?
            .get_mut("when")?
            .get_mut("Discarded")?;
        discarded
            .as_object_mut()?
            .insert("what".into(), Value::Array(Vec::new()));
        Some(())
    }) else {
        return;
    };
    let said: Vec<String> = super::super::super::variants(&plain)
        .iter()
        .filter(|text| text.contains("you discard a card"))
        .flat_map(|text| {
            let kind = text.replacen("you discard a card", &format!("you discard {card}"), 1);
            [kind.replacen(", you create ", ", create ", 1), kind]
        })
        .collect();
    texts.extend(said);
}
