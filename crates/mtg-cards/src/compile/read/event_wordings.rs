//! Wordings of an ability that the oracle text uses and the data does not say: "you" after the event of another
//! player ("Whenever an opponent draws their second card each turn, you draw a card."), "Then" opening a sentence
//! ("..., put a +1/+1 counter on ~. Then ~ deals damage equal to its power ...") and "that creature" for the creature
//! of the event.

use super::pronouns::CONTROLLER_VERBS;
use crate::ability::Ability;

/// The words of a trigger that name a player other than the controller.
const OTHER_PLAYERS: [&str; 6] = [
    "an opponent",
    "each opponent",
    "a player",
    "each player",
    "that player",
    "opponents",
];

/// Adds to `texts` the wordings the oracle uses for the same ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if matches!(ability, Ability::Triggered { .. }) {
        let spelled = you_after_another_player(texts);
        texts.extend(spelled);
        let named = that_creature(texts);
        texts.extend(named);
    }
    let sequenced = then_sentences(texts);
    texts.extend(sequenced);
}

/// "Whenever an opponent draws a card, you draw a card.": the controller is named after the event of another player.
fn you_after_another_player(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter_map(|text| {
            let at = CONTROLLER_VERBS
                .iter()
                .filter_map(|verb| text.find(&format!(", {verb}")))
                .min()?;
            let (head, tail) = text.split_at(at + 2);
            OTHER_PLAYERS
                .iter()
                .any(|player| head.contains(player))
                .then(|| format!("{head}you {tail}"))
        })
        .collect()
}

/// "..., that creature deals damage equal to its power to any target.": the creature of the event, where the data says it.
fn that_creature(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|text| text.contains(", it deals "))
        .map(|text| text.replacen(", it deals ", ", that creature deals ", 1))
        .collect()
}

/// Each sentence after the first, opened with "Then": "Put a +1/+1 counter on ~. Then ~ deals 2 damage to you."
fn then_sentences(texts: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for text in texts {
        for (at, _) in text.match_indices(". ") {
            let (before, after) = text.split_at(at + 2);
            let mut letters = after.chars();
            let Some(first) = letters.next() else {
                continue;
            };
            if first.is_ascii_uppercase() {
                found.push(format!(
                    "{before}Then {}{}",
                    first.to_ascii_lowercase(),
                    letters.as_str()
                ));
            } else if first == '~' {
                found.push(format!("{before}Then {after}"));
            }
        }
    }
    found
}
