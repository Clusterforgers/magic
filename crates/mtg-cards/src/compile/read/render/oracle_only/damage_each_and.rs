//! Damage to two groups in one sentence: "~ deals X damage to each creature without flying and each planeswalker."
//! The renderers write a sentence for each group.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wordings that join the damage to two groups of the same source and amount.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let groups = ability
        .effects()
        .iter()
        .filter(|effect| {
            matches!(
                effect,
                Effect::DealDamage {
                    to: Ref::Each(_) | Ref::EachPlayer,
                    ..
                }
            )
        })
        .count();
    if groups < 2 {
        return;
    }
    let found: Vec<String> = texts.iter().filter_map(|text| joined(text)).collect();
    texts.extend(found);
}

/// "S deals X damage to A. Then S deals X damage to B." as "S deals X damage to A and B", when A and B are groups.
fn joined(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(" damage to ")?;
    let (first, rest) = tail
        .split_once(". Then ~ deals ")
        .or_else(|| tail.split_once(". ~ deals "))?;
    let (amount, second) = rest.split_once(" damage to ")?;
    let deals = head.rsplit_once("deals ")?.1;
    (deals == amount && first.starts_with("each ") && second.starts_with("each "))
        .then(|| format!("{head} damage to {first} and {second}"))
}
