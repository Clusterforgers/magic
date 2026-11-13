//! r55-16 reflexive triggers worded after the steps they follow: "S. When you do, if C, T." (an intervening "if" of the
//! trigger, CR 603.4) and "S. When you do, T." where T is worded as a spell with the trigger's own targets; a pronoun
//! may stand for the source as T's subject ("he deals damage ...").

use serde_json::json;

use crate::ability::Ability;
use crate::effect::Effect;

/// The kinds of ability whose steps a reflexive trigger may follow.
const R5516_HEADS: [&str; 4] = ["Spell", "Triggered", "Activated", "Loyalty"];

/// The ability with only `effects` as its steps.
pub(super) fn r5516_with_steps(ability: &Ability, effects: &[Effect]) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    let key = R5516_HEADS
        .into_iter()
        .find(|key| value.get(*key).is_some())?;
    value[key]["effects"] = json!(effects);
    serde_json::from_value(value).ok()
}

/// The text with its first letter in lower case.
pub(super) fn r5516_lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The wordings of the trigger's steps after "When you do, ": each lowered, and with "~" as their subject also with a
/// pronoun for the source.
pub(super) fn r5516_follow_texts(
    targets: &[crate::refs::TargetSpec],
    steps: &[Effect],
    written: fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    let then = Ability::Spell {
        targets: targets.to_vec(),
        effects: steps.to_vec(),
    };
    let mut texts = Vec::new();
    for text in written(&then).into_iter().take(6) {
        let lowered = r5516_lower_first(&text);
        if let Some(rest) = lowered.strip_prefix("~ ") {
            for pronoun in ["he", "she", "it"] {
                texts.push(format!("{pronoun} {rest}"));
            }
        }
        texts.push(lowered);
    }
    texts
}

/// The intervening "if" of the trigger as its wordings, each ending in ", "; one empty wording without one.
fn r5516_if_texts(condition: Option<&crate::value::Condition>) -> Vec<String> {
    match condition {
        None => vec![String::new()],
        Some(condition) => {
            let mut texts = Vec::new();
            for text in super::wf27_writers::condition_texts(condition) {
                // "you control another red permanent" is also "you control a red permanent other than ~".
                if let Some(rest) = text.strip_prefix("you control another ") {
                    let article = if rest.starts_with(['a', 'e', 'i', 'o', 'u']) {
                        "an"
                    } else {
                        "a"
                    };
                    texts.push(format!("if you control {article} {rest} other than ~, "));
                }
                texts.push(format!("if {text}, "));
            }
            texts
        }
    }
}

/// "S. When you do, if C, T.": an ability whose last step is a reflexive trigger, worded from the ability without it.
pub(super) fn r5516_reflexive_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some((
        Effect::Reflexive {
            targets,
            effects: steps,
            condition,
        },
        before,
    )) = ability.effects().split_last()
    else {
        return;
    };
    if before.is_empty() {
        return;
    }
    let Some(head) = r5516_with_steps(ability, before) else {
        return;
    };
    let ifs = r5516_if_texts(condition.as_ref());
    let follows = r5516_follow_texts(targets, steps, written);
    for head in written(&head).into_iter().take(4) {
        let Some(head) = head.strip_suffix('.') else {
            continue;
        };
        for when in &ifs {
            for follow in &follows {
                let text = format!("{head}. When you do, {when}{follow}");
                if !texts.contains(&text) {
                    texts.push(text);
                }
            }
        }
    }
}
