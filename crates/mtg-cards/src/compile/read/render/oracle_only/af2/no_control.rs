//! A condition that you control none of a kind: "At the beginning of your end step, if you don't control a Food, create a
//! Food token." The renderers write "if you control no Foods".

use super::super::super::{Form, article, phrase_text};
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, Condition};

/// The kind counted by a condition of none, with the plural and the singular words of it.
fn none_of(condition: &Condition) -> Option<(String, String)> {
    let Condition::Compare {
        left: Amount::Count(Selector(preds)),
        op: Cmp::Eq,
        right: Amount::Fixed(0),
    } = condition
    else {
        return None;
    };
    let own = Pred::ControlledBy(Ref::You);
    let kind: Vec<Pred> = preds.iter().filter(|pred| **pred != own).cloned().collect();
    (kind.len() + 1 == preds.len()).then_some(())?;
    let kind = Selector(kind);
    let many = phrase_text(&kind, Form::All)?;
    let one = phrase_text(&kind, Form::Subject)?;
    Some((many, format!("{} {one}", article(&one))))
}

/// Adds the wordings of an intervening "if" that you control none of a kind.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        intervening_if: Some(condition),
        ..
    } = ability
    else {
        return;
    };
    let Some((many, one)) = none_of(condition) else {
        return;
    };
    let (from, into) = (
        format!(", if you control no {many}, "),
        format!(", if you don't control {one}, "),
    );
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(&from))
        .map(|text| text.replacen(&from, &into, 1))
        .collect();
    texts.extend(said);
}
