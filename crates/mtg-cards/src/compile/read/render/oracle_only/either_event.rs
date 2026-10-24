//! Two events of one trigger named in one clause: "Whenever you cycle or discard a card". The renderers word each event
//! of an `AnyOf` apart; the words the two events share are said once.

use super::super::{pattern_text, variants};
use crate::ability::Ability;
use crate::trigger::TriggerPattern;

/// Adds the wordings of a trigger of exactly two events whose clauses differ in the middle only.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::AnyOf(events),
        ..
    } = ability
    else {
        return;
    };
    let [first, second] = events.as_slice() else {
        return;
    };
    let (Some(one), Some(other)) = (pattern_text(first), pattern_text(second)) else {
        return;
    };
    let Some(both) = joined(&one, &other) else {
        return;
    };
    let mut single = ability.clone();
    if let Ability::Triggered { when, .. } = &mut single {
        *when = first.clone();
    }
    let opening = format!("{one}, ");
    for text in variants(&single) {
        if let Some(rest) = text.strip_prefix(&opening) {
            texts.push(format!("{both}, {rest}"));
        }
    }
}

/// "Whenever you cycle a card" and "Whenever you discard a card" as "Whenever you cycle or discard a card".
fn joined(one: &str, other: &str) -> Option<String> {
    let (left, right): (Vec<&str>, Vec<&str>) =
        (one.split(' ').collect(), other.split(' ').collect());
    let shortest = left.len().min(right.len());
    let before = left.iter().zip(&right).take_while(|(a, b)| a == b).count();
    let after = left
        .iter()
        .rev()
        .zip(right.iter().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(shortest - before.min(shortest));
    let (middle, other_middle) = (
        left.get(before..left.len() - after)?,
        right.get(before..right.len() - after)?,
    );
    if before == 0 || middle.is_empty() || other_middle.is_empty() {
        return None;
    }
    Some(format!(
        "{} {} or {}{}{}",
        left[..before].join(" "),
        middle.join(" "),
        other_middle.join(" "),
        if after == 0 { "" } else { " " },
        left[left.len() - after..].join(" ")
    ))
}
