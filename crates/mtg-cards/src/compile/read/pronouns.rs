//! The pronoun wordings of the card in its own effects: a card that attacks, blocks, becomes blocked or enters calls
//! itself "it" in what it does ("Whenever ~ attacks, it gets +1/+1 until end of turn", "put a +1/+1 counter on it"),
//! and a sentence after one that acted on the card opens with "It". The renderer writes `~`, so it also writes the
//! wording with the pronoun.

use super::selectors::{Form, phrase_text};
use super::triggers::{event_creature, itself_acts};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref};

/// The ways every ability names the card at the start of a sentence: `(written with ~, written with the pronoun)`.
const SENTENCE: [(&str, &str); 2] = [(". ~ gets ", ". It gets "), (". ~ gains ", ". It gains ")];

/// The ways a trigger about the card itself names the card: `(written with ~, written with the pronoun)`.
const OF_THE_EVENT: [(&str, &str); 9] = [
    (", ~ fights ", ", it fights "),
    (", attach ~ to ", ", attach it to "),
    (
        ", return ~ to its owner's hand",
        ", return it to its owner's hand",
    ),
    (
        ", shuffle ~ into its owner's library",
        ", shuffle it into its owner's library",
    ),
    (", ~ gets ", ", it gets "),
    (", ~ gains ", ", it gains "),
    (
        ", ~ can't be blocked this turn",
        ", it can't be blocked this turn",
    ),
    (" counter on ~", " counter on it"),
    (" counters on ~", " counters on it"),
];

/// Adds to `texts` the wording "Those creatures" for a group a sentence before counted ("Draw a card for each creature
/// you control with a +1/+1 counter on it. Those creatures gain indestructible until end of turn.").
pub(super) fn of_the_group(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { effects, .. } = ability else {
        return;
    };
    let mut pronouns = Vec::new();
    for effect in effects {
        let Effect::Continuous {
            affected: Ref::Each(group),
            ..
        } = effect
        else {
            continue;
        };
        let Some(Pred::Type(_)) = group.0.first() else {
            continue;
        };
        let (Some(plural), Some(single)) = (
            phrase_text(group, Form::All),
            phrase_text(group, Form::Each),
        ) else {
            continue;
        };
        let Some(noun) = plural.split(' ').next() else {
            continue;
        };
        let mut letters = plural.chars();
        let capital: String = letters
            .next()
            .map(|first| first.to_uppercase().chain(letters).collect())
            .unwrap_or_default();
        let (named, those) = (format!(". {capital} "), format!(". Those {noun} "));
        for text in texts.iter() {
            if text.contains(&format!("each {single}")) && text.contains(&named) {
                pronouns.push(text.replacen(&named, &those, 1));
            }
        }
    }
    texts.extend(pronouns);
}

/// Adds to `texts` the wording that calls the card "it", for each text that has one.
pub(super) fn of_the_card(ability: &Ability, texts: &mut Vec<String>) {
    let itself = matches!(
        ability,
        Ability::Triggered { when, .. }
            if event_creature(when) == Some(Ref::This) || itself_acts(when)
    );
    let pronouns: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let mut found = text.clone();
            let own = if itself { &OF_THE_EVENT[..] } else { &[] };
            for (named, pronoun) in SENTENCE.iter().chain(own) {
                found = found.replace(named, pronoun);
            }
            (found != *text).then_some(found)
        })
        .collect();
    texts.extend(pronouns);
}

/// The player of a trigger is "they" in what it does: `(that player and a verb, they and the verb)`.
const THEY: [(&str, &str); 6] = [
    ("that player loses ", "they lose "),
    ("that player gains ", "they gain "),
    ("that player draws ", "they draw "),
    ("that player discards ", "they discard "),
    ("that player mills ", "they mill "),
    ("that player sacrifices ", "they sacrifice "),
];

/// Adds to `texts` the wording "they lose 2 life" for "that player loses 2 life" in a trigger.
pub(super) fn they(ability: &Ability, texts: &mut Vec<String>) {
    if !matches!(ability, Ability::Triggered { .. }) {
        return;
    }
    let spelled: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let found = THEY.iter().fold(text.clone(), |found, (named, they)| {
                found.replace(named, they)
            });
            (found != *text).then_some(found)
        })
        .collect();
    texts.extend(spelled);
}

/// The verbs an effect of the controller opens with.
pub(super) const CONTROLLER_VERBS: [&str; 12] = [
    "scry ",
    "surveil ",
    "create ",
    "draw ",
    "gain ",
    "lose ",
    "mill ",
    "put ",
    "add ",
    "discard ",
    "investigate",
    "proliferate",
];

/// Adds to `texts` the wording "you scry 2" for a trigger on the creature an Aura or an Equipment is attached to: the
/// controller of the attachment is not the controller of the creature, so the card says whom the effect is for
/// ("Whenever enchanted creature attacks, you scry 2.").
pub(super) fn you_after_the_attached(ability: &Ability, texts: &mut Vec<String>) {
    if !matches!(ability, Ability::Triggered { .. }) {
        return;
    }
    let spelled: Vec<String> = texts
        .iter()
        .filter(|text| {
            text.starts_with("Whenever equipped creature ")
                || text.starts_with("Whenever enchanted creature ")
        })
        .filter_map(|text| {
            let (head, tail) = text.split_once(", ")?;
            CONTROLLER_VERBS
                .iter()
                .any(|verb| tail.starts_with(verb))
                .then(|| format!("{head}, you {tail}"))
        })
        .collect();
    texts.extend(spelled);
}

/// Adds to `texts` the wording "That creature gains shroud until end of turn" for the creature a trigger attached the card
/// to ("When this Equipment enters, attach it to target creature you control. That creature gains shroud until end of turn.").
pub(super) fn that_creature_after_an_attach(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    let attached = matches!(
        effects.first(),
        Some(Effect::Attach {
            to: Ref::Target(0),
            ..
        })
    );
    if !attached {
        return;
    }
    let spelled: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            SENTENCE.iter().filter_map(|(_, pronoun)| {
                let named = pronoun.replace(". It ", ". That creature ");
                text.contains(pronoun)
                    .then(|| text.replacen(pronoun, &named, 1))
            })
        })
        .collect();
    texts.extend(spelled);
}
