//! A trigger whose one effect lasts until its controller's next turn, with the duration after the effect as the card
//! prints it: "When this creature enters, target creature an opponent controls can't attack or block until your next
//! turn." The renderers put the duration before the whole sentence, trigger included.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::Duration;

/// The duration as the renderers open the sentence with it.
const OPENING: &str = "Until your next turn, ";

/// Adds the wording with the duration at the end of a one-sentence trigger written with it in front.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    let [
        Effect::Continuous {
            duration: Duration::UntilYourNextTurn,
            ..
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let more: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let rest = text.strip_prefix(OPENING)?;
            let sentence = rest.strip_suffix('.')?;
            let opens_trigger = ["when ", "whenever ", "at "]
                .iter()
                .any(|word| sentence.starts_with(word));
            (opens_trigger && !sentence.contains(". ") && !sentence.contains('"')).then(|| {
                let mut letters = sentence.chars();
                let first = letters.next().map(|c| c.to_uppercase().collect::<String>());
                format!(
                    "{}{} until your next turn.",
                    first.unwrap_or_default(),
                    letters.as_str()
                )
            })
        })
        .collect();
    for text in more {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
