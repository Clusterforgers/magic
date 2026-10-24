//! An effect that another replaces when a condition holds: "Search your library for a basic land card, put that card onto
//! the battlefield, then shuffle. If this spell was kicked, instead search your library for up to two basic land cards,
//! put them onto the battlefield, then shuffle." and "Draw a card. If you control eight or more lands, draw two cards
//! instead." (an `Effect::If` of the condition over the second sentence, with the first as its `otherwise`), read after
//! the first sentence and written back as `instead` before or after the second.

use super::conditional;
use super::effects;
use super::tokens::Reader;
use crate::effect::Effect;

/// ". If C, instead S2." or ". If C, S2 instead." after the sentence `first` that was read: the `If` over the second
/// sentence in place of `first`. The sentences name no target, so the two branches cannot disagree on what is chosen.
pub(super) fn tail(r: &mut Reader<'_>, first: &[Effect]) -> Option<Effect> {
    r.targets.is_empty().then_some(())?;
    r.attempt(|r| {
        r.need("if")?;
        let condition = conditional::if_condition(r)?;
        r.punct(',').then_some(())?;
        let before = r.word("instead");
        let then = effects::closed_shape(r, |r| (before || r.word("instead")) && r.sentence_end())?;
        r.targets.is_empty().then_some(())?;
        Some(Effect::If {
            condition,
            then,
            otherwise: first.to_vec(),
        })
    })
}

/// The wording with `instead` after the second sentence for a text that says it before: ". If C, instead S2." is
/// ". If C, S2 instead." too.
pub fn other_wordings(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(". If ")?;
            let (condition, second) = rest.split_once(", instead ")?;
            let second = second.strip_suffix('.').unwrap_or(second);
            Some(format!("{head}. If {condition}, {second} instead."))
        })
        .collect()
}
