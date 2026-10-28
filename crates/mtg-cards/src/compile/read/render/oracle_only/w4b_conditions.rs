//! Static abilities under a condition the renderers do not word ("~ has indestructible as long as it attacked this
//! turn.", "As long as you've cast two or more noncreature spells this turn, ~ has double strike."): the ability is
//! written under "it's your turn" and the condition's words are put in its place.

use super::w4b_stand::{json, stand, swapped, word};
use crate::ability::Ability;
use serde_json::{Value, json};

/// Adds the wordings of a static ability whose condition one of the phrases words.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static {
        condition: Some(condition),
        ..
    } = ability
    else {
        return;
    };
    let Some((trailing, leading)) = phrases(&json(condition)) else {
        return;
    };
    let Some(stand) = stand(ability, |value| {
        *value.pointer_mut("/Static/condition")? = json!("YourTurn");
        Some(())
    }) else {
        return;
    };
    let trailing = format!("as long as {trailing}");
    let leading = format!("As long as {leading}");
    texts.extend(swapped(&stand, &[("as long as it's your turn", &trailing)]));
    texts.extend(swapped(&stand, &[("As long as it's your turn", &leading)]));
}

/// The words of the condition after "as long as" at the end of the sentence and at its head: the card is "it" after
/// its own name and "~" before it.
fn phrases(condition: &Value) -> Option<(String, String)> {
    if *condition == json!({"Matches": {"subject": "This", "selector": ["AttackedThisTurn"]}}) {
        return Some((
            "it attacked this turn".to_owned(),
            "~ attacked this turn".to_owned(),
        ));
    }
    let compare = condition.get("Compare")?;
    (compare.get("op")? == "Ge").then_some(())?;
    let count = word(compare.get("right")?)?;
    let phrase = at_least(compare.get("left")?, count)?;
    Some((phrase.clone(), phrase))
}

/// "you've cast two or more noncreature spells this turn" for an amount of at least `count`.
fn at_least(left: &Value, count: &str) -> Option<String> {
    if *left == json!({"Tallied": {"who": "You", "what": "NoncreatureSpellsCast"}}) {
        return Some(format!(
            "you've cast {count} or more noncreature spells this turn"
        ));
    }
    if *left == json!({"AnyCounters": "This"}) {
        return Some(format!("~ has {count} or more counters on it"));
    }
    if *left == json!({"Counters": {"on": "Opponents", "counter": "poison"}}) {
        return Some(format!("an opponent has {count} or more poison counters"));
    }
    None
}
