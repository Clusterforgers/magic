//! A target that is not the source: "Target creature other than this creature has base power and toughness 0/2 until end
//! of turn." The renderers write "another target creature" and the duration first.

use crate::ability::Ability;
use crate::refs::{Pred, Targetable};

const OPENING: &str = "Until end of turn, ";

/// The sentence that follows the duration with the duration at its end.
fn duration_last(text: &str) -> Option<String> {
    let at = text.find(OPENING)?;
    let (head, rest) = (&text[..at], &text[at + OPENING.len()..]);
    let (sentence, tail) = match rest.split_once(". ") {
        Some((sentence, tail)) => (sentence, format!(". {tail}")),
        None => (rest.strip_suffix('.')?, ".".to_owned()),
    };
    let mut letters = sentence.chars();
    let first = letters.next()?.to_uppercase().collect::<String>();
    Some(format!(
        "{head}{first}{} until end of turn{tail}",
        letters.as_str()
    ))
}

/// Adds the wordings of an ability whose one target is any creature but the source.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let others = ability.targets().iter().any(|spec| {
        matches!(&spec.what, Targetable::Object(selector)
            if selector.0.contains(&Pred::Other))
    });
    if !others {
        return;
    }
    let (from, into) = (
        "another target creature",
        "target creature other than this creature",
    );
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(from))
        .flat_map(|text| {
            let named = text.replacen(from, into, 1);
            let last = duration_last(&named);
            [Some(named), last].into_iter().flatten()
        })
        .collect();
    texts.extend(said);
}
