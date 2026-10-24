//! Steps that follow a step which took something away, worded as the card links them: "Sacrifice it. When you do,
//! search ...", "Remove all flood counters from ~. If no counters were removed this way, ... Otherwise, ...", and a
//! payment that buys a search ("you may sacrifice a land. If you do, search ..., put them onto the battlefield tapped,
//! then shuffle"). The renderers write the steps of a search only as the steps of an ability, so the steps after the link
//! are worded as a spell of their own.

use super::super::variants;
use super::bare::{lowered, with_effects};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::{Amount, Condition};

/// Adds the wordings of an ability whose last step is a link on the step before it, or a payment with several steps.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let effects = ability.effects();
    match effects.split_last() {
        Some((
            Effect::If {
                condition,
                then,
                otherwise,
            },
            before,
        )) if before.last().is_some_and(|last| {
            matches!(
                last,
                Effect::Sacrifice { .. } | Effect::RemoveCounters { .. }
            )
        }) =>
        {
            linked(ability, before, condition, then, otherwise, texts);
        }
        Some((
            Effect::Pay {
                who,
                cost,
                times,
                paid,
                unpaid,
            },
            before,
        )) if unpaid.is_empty() && paid.len() > 1 => {
            let stand_in = Effect::Pay {
                who: who.clone(),
                cost: cost.clone(),
                times: times.clone(),
                paid: vec![Effect::Draw {
                    who: Ref::You,
                    count: Amount::one(),
                }],
                unpaid: Vec::new(),
            };
            let mut all = before.to_vec();
            all.push(stand_in);
            payment(ability, all, paid, texts);
        }
        _ => {}
    }
}

/// The steps as a spell of their own: the sentences of the wordings of an ability that has no trigger or cost around them.
fn steps(effects: &[Effect]) -> Vec<String> {
    variants(&Ability::Spell {
        targets: Vec::new(),
        effects: effects.to_vec(),
    })
}

/// "Sacrifice it. When you do, S." and "Remove all ... If no counters were removed this way, S. Otherwise, S."
fn linked(
    ability: &Ability,
    before: &[Effect],
    condition: &Condition,
    then: &[Effect],
    otherwise: &[Effect],
    texts: &mut Vec<String>,
) {
    let Some(head) = with_effects(ability, before.to_vec()) else {
        return;
    };
    let wanted = match (condition, otherwise.is_empty()) {
        (Condition::Performed, true) => "When you do, ",
        (Condition::Not(inner), _) if **inner == Condition::Performed => {
            "If no counters were removed this way, "
        }
        _ => return,
    };
    let remove = matches!(before.last(), Some(Effect::RemoveCounters { .. }));
    if wanted.starts_with("If no") != remove {
        return;
    }
    let heads = variants(&head).into_iter().flat_map(|text| {
        let all = remove_all(&text);
        std::iter::once(text).chain(all)
    });
    for head in heads {
        for body in and_joined(steps(then)) {
            let text = format!("{head} {wanted}{}", lowered(&body));
            if otherwise.is_empty() {
                texts.push(text);
                continue;
            }
            for other in and_joined(steps(otherwise)) {
                texts.push(format!("{text} Otherwise, {}", lowered(&other)));
            }
        }
    }
}

/// The wordings and, for two sentences, the one step joined to the other by "and".
fn and_joined(bodies: Vec<String>) -> Vec<String> {
    let joined: Vec<String> = bodies
        .iter()
        .filter_map(|body| {
            let (first, second) = body.strip_suffix('.')?.split_once(". ")?;
            (!second.contains(". ")).then(|| format!("{first} and {}.", lowered(second)))
        })
        .collect();
    bodies.into_iter().chain(joined).collect()
}

/// "remove X flood counters from ~, where X is the number of flood counters on ~" as "remove all flood counters from ~".
fn remove_all(text: &str) -> Option<String> {
    let (before, rest) = text.split_once("remove X ")?;
    let (kind, rest) = rest.split_once(" counters from ~, where X is the number of ")?;
    let tail = rest.strip_prefix(kind)?.strip_prefix(" counters on ~")?;
    Some(format!("{before}remove all {kind} counters from ~{tail}"))
}

/// "..., you may sacrifice a land. If you do, S." where S is several steps: the stand-in step is swapped for them.
fn payment(ability: &Ability, all: Vec<Effect>, paid: &[Effect], texts: &mut Vec<String>) {
    let Some(head) = with_effects(ability, all) else {
        return;
    };
    for head in variants(&head) {
        let Some(opening) = head.strip_suffix("draw a card.") else {
            continue;
        };
        for body in steps(paid) {
            texts.push(format!("{opening}{}", lowered(&body)));
        }
    }
}
