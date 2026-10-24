//! Older cards call the opponents "each other player" and let a player sacrifice "of their choice": "Whenever a
//! creature you control dies, each other player sacrifices a creature of their choice."

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// The verbs after "each opponent" that the older wording shares.
const VERBS: [&str; 6] = ["sacrifices", "discards", "loses", "gains", "draws", "mills"];

/// Adds the wordings with "each other player" for each opponent, and "of their choice" after a sacrifice.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !ability.effects().iter().any(by_opponents) {
        return;
    }
    let mut found = Vec::new();
    for verb in VERBS {
        // Inside a sentence and opening one.
        for (new, old) in [
            (
                format!("each other player {verb} "),
                format!("each opponent {verb} "),
            ),
            (
                format!("Each other player {verb} "),
                format!("Each opponent {verb} "),
            ),
        ] {
            found.extend(
                texts
                    .iter()
                    .filter(|text| text.contains(&old))
                    .map(|text| text.replacen(&old, &new, 1)),
            );
        }
    }
    let chosen: Vec<String> = found
        .iter()
        .chain(texts.iter())
        .filter_map(|text| of_their_choice(text))
        .collect();
    texts.extend(found);
    texts.extend(chosen);
}

/// Whether the effect is done by every opponent.
fn by_opponents(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::Sacrifice {
            who: Ref::Opponents,
            ..
        } | Effect::Discard {
            who: Ref::Opponents,
            ..
        } | Effect::Mill {
            who: Ref::Opponents,
            ..
        } | Effect::LoseLife {
            who: Ref::Opponents,
            ..
        } | Effect::GainLife {
            who: Ref::Opponents,
            ..
        } | Effect::Draw {
            who: Ref::Opponents,
            ..
        }
    )
}

/// The text with "of their choice" at the end of the sentence of a sacrifice of one object.
fn of_their_choice(text: &str) -> Option<String> {
    let at = text.find(" sacrifices a")?;
    let end = at + text[at..].find('.')?;
    let sentence = &text[at..end];
    (!sentence.contains(" of their choice"))
        .then(|| format!("{} of their choice{}", &text[..end], &text[end..]))
}
