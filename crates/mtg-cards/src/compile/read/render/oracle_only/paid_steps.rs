//! The two steps a payment buys are joined by "and" on a card: "you may pay {1}. If you do, put a +1/+1 counter on this
//! creature and create a 1/1 blue Tentacle creature token." The renderers write ", then" between them.

use crate::ability::Ability;
use crate::effect::Effect;

/// Adds the wordings that join the two steps after "If you do" by "and".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let two_steps = ability
        .effects()
        .iter()
        .any(|effect| matches!(effect, Effect::Pay { paid, .. } if paid.len() == 2));
    if !two_steps {
        return;
    }
    let found: Vec<String> = texts.iter().filter_map(|text| joined(text)).collect();
    texts.extend(found);
}

/// The text with ", then" between the two steps after "If you do" as " and".
fn joined(text: &str) -> Option<String> {
    let at = text.find(". If you do, ")?;
    let (head, steps) = text.split_at(at);
    let (first, second) = steps.split_once(", then ")?;
    Some(format!("{head}{first} and {second}"))
}
