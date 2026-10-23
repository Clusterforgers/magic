//! The sentence that limits when an ability is activated: "Activate only if you control a Swamp.", "Activate only during
//! your turn and only if a card left your graveyard this turn.", "Activate only as a sorcery and only once each turn." The
//! parts (a condition, a timing, a limit) come in any order on the cards. Written from the data only.

use super::as_long_as;
use super::statics::control_texts;
use crate::ability::{Limit, Timing};
use crate::value::Condition;

/// The sentences for an ability with a condition, in every order of its parts.
pub(super) fn tails(timing: Timing, condition: &Condition, limit: Option<Limit>) -> Vec<String> {
    let mut parts: Vec<Vec<String>> = Vec::new();
    if timing == Timing::Sorcery {
        parts.push(vec!["as a sorcery".to_owned()]);
    }
    if limit.is_some() {
        parts.push(vec!["once each turn".to_owned()]);
    }
    let held = held(condition);
    if held.is_empty() {
        return Vec::new();
    }
    parts.push(held);
    orders(&parts)
        .into_iter()
        .map(|order| format!("Activate only {}.", order.join(" and only ")))
        .collect()
}

/// The condition after "only": "during your turn", or "if" and what holds.
fn held(condition: &Condition) -> Vec<String> {
    if *condition == Condition::YourTurn {
        return vec!["during your turn".to_owned()];
    }
    let mut said = control_texts(condition);
    said.extend(as_long_as::texts(condition));
    said.into_iter().map(|held| format!("if {held}")).collect()
}

/// Every order of the parts, each part in each of its wordings.
fn orders(parts: &[Vec<String>]) -> Vec<Vec<String>> {
    let mut found: Vec<Vec<String>> = Vec::new();
    permute(parts, &mut Vec::new(), &mut found);
    found
}

fn permute(parts: &[Vec<String>], chosen: &mut Vec<usize>, found: &mut Vec<Vec<String>>) {
    if chosen.len() == parts.len() {
        let mut texts: Vec<Vec<String>> = vec![Vec::new()];
        for &at in chosen.iter() {
            texts = texts
                .iter()
                .flat_map(|so_far| {
                    parts[at].iter().map(move |part| {
                        let mut next = so_far.clone();
                        next.push(part.clone());
                        next
                    })
                })
                .collect();
        }
        found.extend(texts);
        return;
    }
    for at in 0..parts.len() {
        if !chosen.contains(&at) {
            chosen.push(at);
            permute(parts, chosen, found);
            chosen.pop();
        }
    }
}
