//! Every player sacrifices several of a kind: "Each player sacrifices two creatures.", "Each player sacrifices three
//! creatures of their choice. You create three Food tokens." The renderers write the sacrifice of one object by each
//! opponent; the sentence is derived from it with the players, the number and the plural of the kind.

use super::super::super::facts;
use super::super::variants;
use super::bare::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// The opening of the sentence the renderers write for each opponent.
const OPPONENT: &str = "Each opponent sacrifices ";

/// Adds the wordings of a sacrifice of several permanents by each player (or of two or more by each opponent).
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let effects = ability.effects();
    let Some((at, number)) = effects.iter().enumerate().find_map(|(at, effect)| {
        match effect {
            Effect::Sacrifice {
                who: Ref::EachPlayer,
                count: Amount::Fixed(number),
                ..
            } => u32::try_from(*number).ok().filter(|n| *n >= 1),
            Effect::Sacrifice {
                who: Ref::Opponents,
                count: Amount::Fixed(number),
                ..
            } => u32::try_from(*number).ok().filter(|n| *n >= 2),
            _ => None,
        }
        .map(|number| (at, number))
    }) else {
        return;
    };
    let Effect::Sacrifice { who, filter, .. } = &effects[at] else {
        return;
    };
    let mut stand_in = effects.to_vec();
    stand_in[at] = Effect::Sacrifice {
        who: Ref::Opponents,
        filter: filter.clone(),
        count: Amount::one(),
    };
    let Some(bare) = with_effects(ability, stand_in) else {
        return;
    };
    let players = if *who == Ref::EachPlayer {
        "Each player sacrifices "
    } else {
        OPPONENT
    };
    for text in variants(&bare) {
        for words in facts::quantity_texts(number) {
            texts.extend(sentence(&text, players, &words, number));
        }
    }
}

/// The text with the sacrifice of one object by each opponent worded for `players` and `words` objects.
fn sentence(text: &str, players: &str, words: &str, number: u32) -> Vec<String> {
    let Some(start) = text.find(OPPONENT) else {
        return Vec::new();
    };
    if start != 0 && !text[..start].ends_with(". ") {
        return Vec::new();
    }
    let after = &text[start + OPPONENT.len()..];
    let Some(noun) = after
        .strip_prefix("an ")
        .or_else(|| after.strip_prefix("a "))
    else {
        return Vec::new();
    };
    let end = noun.find(['.', ' ']).unwrap_or(noun.len());
    let (kind, rest) = noun.split_at(end);
    // One word for the kind; a longer phrase ("creature or planeswalker") has a plural of its own.
    if !(rest.starts_with('.') || rest.starts_with(" of their choice")) {
        return Vec::new();
    }
    let kind = if number == 1 {
        let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
            "an"
        } else {
            "a"
        };
        format!("{article} {kind}")
    } else if kind.chars().all(|c| c.is_ascii_lowercase()) {
        format!("{words} {kind}s")
    } else {
        return Vec::new();
    };
    let head = format!("{}{players}{kind}", &text[..start]);
    vec![
        format!("{head}{rest}"),
        match rest.strip_prefix(" of their choice") {
            Some(tail) => format!("{head}{tail}"),
            None => format!("{head} of their choice{rest}"),
        },
    ]
}
