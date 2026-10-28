//! Conditional steps as the oracle words them: "If X, Y.", "Y if X.", "Otherwise, Z.", "If you can't, Z." and "Then
//! if X, Y.". Each `If` of the ability stands as a marker draw while the ability is written, and the marker's sentence
//! is replaced by the wordings of the `If`, whose steps are written in place between two markers so that their
//! references ("it", "that card", "those tokens") read as in the ability.

use super::super::super::amounts::count_word;
use super::super::{capitalize, written_variants};
use super::twin_json::lowered;
use super::w4a_if_conditions::w4a_condition_texts;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// The marker draw that opens the steps of an `If` written in place.
const W4A_OPEN: i32 = 19;
/// The marker draw that closes them.
const W4A_CLOSE: i32 = 20;
/// The marker draw of the first effect; the effect at index `i` is `W4A_FIRST + i`.
const W4A_FIRST: i32 = 11;
/// The most wordings kept for one ability.
const W4A_MOST: usize = 4096;

/// Adds the wordings of an ability with a conditional step.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    texts.extend(w4a_if_wordings(ability));
}

/// The draw that stands for a step while the ability is written.
fn w4a_marker(number: i32) -> Effect {
    Effect::Draw {
        who: Ref::You,
        count: Amount::Fixed(number),
    }
}

/// The ability with other effects.
fn w4a_with_effects(ability: &Ability, effects: Vec<Effect>) -> Option<Ability> {
    let mut twin = ability.clone();
    match &mut twin {
        Ability::Spell { effects: own, .. }
        | Ability::Activated { effects: own, .. }
        | Ability::Triggered { effects: own, .. }
        | Ability::Loyalty { effects: own, .. } => *own = effects,
        _ => return None,
    }
    Some(twin)
}

/// The effects with every `If` before `end` replaced by its marker.
fn w4a_marked(effects: &[Effect], end: usize) -> Vec<Effect> {
    effects
        .iter()
        .take(end)
        .zip(W4A_FIRST..)
        .map(|(effect, number)| match effect {
            Effect::If { .. } => w4a_marker(number),
            other => other.clone(),
        })
        .collect()
}

/// The wordings of the ability with each top-level `If` worded by [`w4a_if_texts`].
fn w4a_if_wordings(ability: &Ability) -> Vec<String> {
    let effects = ability.effects();
    let ifs: Vec<usize> = (0..effects.len())
        .filter(|&i| matches!(effects.get(i), Some(Effect::If { .. })))
        .collect();
    if ifs.is_empty() || effects.len() > 7 {
        return Vec::new();
    }
    let Some(marked) = w4a_with_effects(ability, w4a_marked(effects, effects.len())) else {
        return Vec::new();
    };
    let mut out = written_variants(&marked);
    for &i in &ifs {
        let alternatives = w4a_if_texts(ability, i);
        let Some(number) = i32::try_from(i).ok().map(|i| W4A_FIRST + i) else {
            return Vec::new();
        };
        out = out
            .iter()
            .flat_map(|text| w4a_substitute(text, number, &alternatives, i > 0))
            .take(W4A_MOST)
            .collect();
    }
    out
}

/// The text with the marker's sentence replaced by each alternative; nothing when the marker is not a sentence of its
/// own. After another step, "Then if" may open the sentence.
fn w4a_substitute(text: &str, number: i32, alternatives: &[String], after: bool) -> Vec<String> {
    let capital = format!("Draw {} cards.", w4a_word(number));
    let lower = format!("draw {} cards.", w4a_word(number));
    if let Some((head, tail)) = text.split_once(&capital) {
        let mut out = Vec::new();
        for alternative in alternatives {
            out.push(format!("{head}{}.{tail}", capitalize(alternative)));
            if after && alternative.starts_with("if ") {
                out.push(format!("{head}Then {alternative}.{tail}"));
            }
        }
        return out;
    }
    let Some((head, tail)) = text.split_once(&lower) else {
        return Vec::new();
    };
    if !(head.ends_with(", ") || head.ends_with(": ")) {
        return Vec::new();
    }
    alternatives
        .iter()
        .map(|alternative| format!("{head}{alternative}.{tail}"))
        .collect()
}

/// The wordings of the `If` at `i` without the period at their end: "if X, Y", "Y if X" and either with ". Otherwise,
/// Z", or "Z. If X, Y instead".
fn w4a_if_texts(ability: &Ability, i: usize) -> Vec<String> {
    let effects = ability.effects();
    let Some(Effect::If {
        condition,
        then,
        otherwise,
    }) = effects.get(i)
    else {
        return Vec::new();
    };
    // After another conditional sentence, "Otherwise" reads as the other branch of that one.
    if !otherwise.is_empty() && i > 0 && matches!(effects.get(i - 1), Some(Effect::If { .. })) {
        return Vec::new();
    }
    let conditions = w4a_condition_texts(condition);
    let thens = w4a_steps_texts(ability, i, then);
    let mut out = Vec::new();
    for condition in &conditions {
        for then in &thens {
            out.push(format!("if {condition}, {then}"));
            if !then.contains(". ") {
                out.push(format!("{then} if {condition}"));
            }
        }
    }
    if otherwise.is_empty() {
        return out;
    }
    let others = w4a_steps_texts(ability, i, otherwise);
    // A step on another condition otherwise opens a sentence of its own: "If it was a land card, ...".
    let chained = matches!(otherwise.as_slice(), [Effect::If { .. }]);
    let mut with_others: Vec<String> = out
        .iter()
        .flat_map(|text| {
            others.iter().flat_map(move |other| {
                let mut both = vec![format!("{text}. Otherwise, {other}")];
                if chained && other.starts_with("if ") {
                    both.push(format!("{text}. {}", capitalize(other)));
                }
                both
            })
        })
        .collect();
    with_others.extend(super::w4a_if_instead::w4a_instead_texts(
        &conditions,
        then,
        otherwise,
        &others,
    ));
    with_others
}

/// The wordings of `steps` written in place of the `If` at `i`: one sentence opening in lower case without its period,
/// or a sentence followed by sentences that open with "Otherwise," (a nested `If` with its other branch).
fn w4a_steps_texts(ability: &Ability, i: usize, steps: &[Effect]) -> Vec<String> {
    let effects = ability.effects();
    let mut placed = w4a_marked(effects, i);
    placed.push(w4a_marker(W4A_OPEN));
    placed.extend(steps.iter().cloned());
    placed.push(w4a_marker(W4A_CLOSE));
    let Some(twin) = w4a_with_effects(ability, placed) else {
        return Vec::new();
    };
    let open = format!("raw {} cards. ", w4a_word(W4A_OPEN));
    let close = format!(" Draw {} cards.", w4a_word(W4A_CLOSE));
    let mut out: Vec<String> = written_variants(&twin)
        .iter()
        .filter_map(|text| {
            let (_, rest) = text.rsplit_once(&open)?;
            let (inner, _) = rest.split_once(&close)?;
            let inner = inner.strip_suffix('.')?;
            let mut sentences = inner.split(". ");
            let first = sentences.next()?;
            let plain = !first.starts_with("Then ")
                && !(W4A_FIRST..=W4A_CLOSE)
                    .any(|number| inner.contains(&format!(" {} cards", w4a_word(number))));
            (plain && sentences.all(|later| later.starts_with("Otherwise, ")))
                .then(|| lowered(inner))
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The word of a marker's number ("eleven").
fn w4a_word(number: i32) -> &'static str {
    u32::try_from(number)
        .ok()
        .and_then(count_word)
        .unwrap_or("none")
}
