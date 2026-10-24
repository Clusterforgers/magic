//! A condition of a step in the resolution follows the step on a card: "put a +1/+1 counter on each creature you control
//! if you control a creature with power 5 or greater. Then you gain 10 life if ...". The renderers put it first.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::value::Condition;

/// The most sentences of a text whose condition moves; each subset of them is a wording.
const MOST_MOVED: usize = 4;

/// Adds the wordings that put the condition of a step after it, for any subset of the steps with one.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let intervening = matches!(
        ability,
        Ability::Triggered {
            intervening_if: Some(_),
            ..
        }
    );
    let conditional = ability.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::If { condition, otherwise, .. }
                if otherwise.is_empty() && *condition != Condition::Performed
        )
    });
    if intervening || !conditional {
        return;
    }
    let found: Vec<String> = texts.iter().flat_map(|text| moved(text)).collect();
    texts.extend(found);
}

/// The wordings of the text with the condition of some of its sentences after the step.
fn moved(text: &str) -> Vec<String> {
    let Some(body) = text.strip_suffix('.') else {
        return Vec::new();
    };
    let sentences: Vec<&str> = body.split(". ").collect();
    let flipped: Vec<Option<String>> = sentences.iter().map(|s| trailing(s)).collect();
    let at: Vec<usize> = (0..flipped.len())
        .filter(|i| flipped[*i].is_some())
        .collect();
    if at.is_empty() || at.len() > MOST_MOVED {
        return Vec::new();
    }
    (1..1u32 << at.len())
        .map(|mask| {
            let parts: Vec<String> = sentences
                .iter()
                .enumerate()
                .map(|(i, sentence)| {
                    let bit = at.iter().position(|j| *j == i);
                    match (bit, &flipped[i]) {
                        (Some(bit), Some(flip)) if mask & (1 << bit) != 0 => flip.clone(),
                        _ => (*sentence).to_owned(),
                    }
                })
                .collect();
            format!("{}.", parts.join(". "))
        })
        .collect()
}

/// A sentence "..., if C, E" or "Then if C, E" or "If C, E" as "..., E if C", "Then E if C" and "E if C".
fn trailing(sentence: &str) -> Option<String> {
    let (lead, rest) = if let Some(rest) = sentence.strip_prefix("Then if ") {
        ("Then ".to_owned(), rest)
    } else if let Some(rest) = sentence.strip_prefix("If ") {
        (String::new(), rest)
    } else {
        let at = sentence.find(", if ")?;
        (sentence[..at + 2].to_owned(), &sentence[at + 5..])
    };
    let (condition, step) = rest.split_once(", ")?;
    if step.starts_with("instead") || condition.contains(',') {
        return None;
    }
    let step = if lead.is_empty() {
        let mut chars = step.chars();
        let first = chars.next()?;
        format!("{}{}", first.to_uppercase(), chars.as_str())
    } else {
        step.to_owned()
    };
    Some(format!("{lead}{step} if {condition}"))
}
