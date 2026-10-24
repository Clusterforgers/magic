//! The sentence that limits when an ability is activated: "Activate only if you control a Swamp.", "Activate only during
//! your turn and only if a card left your graveyard this turn.", "Activate only during your upkeep.", "Activate only as a
//! sorcery and only once each turn." The parts (a condition, a timing, a limit) come in any order on the cards. Read by
//! `restrictions.rs`.

use super::as_long_as;
use super::restrictions::moments;
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
    let Some(held) = held(condition) else {
        return Vec::new();
    };
    parts.extend(held);
    orders(&parts)
        .into_iter()
        .map(|order| format!("Activate only {}.", order.join(" and only ")))
        .collect()
}

/// The conditions in one list, an `All` flattened.
fn flat<'a>(condition: &'a Condition, into: &mut Vec<&'a Condition>) {
    match condition {
        Condition::All(all) => all.iter().for_each(|inner| flat(inner, into)),
        other => into.push(other),
    }
}

/// The parts after "only", each in its wordings: a moment of the turn ("during your upkeep", with "before attackers are
/// declared" after it), or "if" and a state of the game; `None` when a condition has no wording.
fn held(condition: &Condition) -> Option<Vec<Vec<String>>> {
    let mut list = Vec::new();
    flat(condition, &mut list);
    let mut known = moments();
    known.sort_by_key(|(_, conditions)| std::cmp::Reverse(conditions.len()));
    let mut parts = Vec::new();
    let mut at = 0;
    while at < list.len() {
        let moment = known.iter().find(|(_, conditions)| {
            list.get(at..at + conditions.len())
                .is_some_and(|found| found.iter().zip(conditions).all(|(a, b)| *a == b))
        });
        if let Some((words, conditions)) = moment {
            at += conditions.len();
            let mut said = format!("during {words}");
            if list.get(at) == Some(&&Condition::BeforeAttackers) {
                said.push_str(", before attackers are declared");
                at += 1;
            }
            parts.push(vec![said]);
        } else if *list[at] == Condition::BeforeAttackers {
            parts.push(vec!["before attackers are declared".to_owned()]);
            at += 1;
        } else {
            let mut said = control_texts(list[at]);
            said.extend(as_long_as::texts(list[at]));
            if said.is_empty() {
                return None;
            }
            parts.push(said.into_iter().map(|held| format!("if {held}")).collect());
            at += 1;
        }
    }
    Some(parts)
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
