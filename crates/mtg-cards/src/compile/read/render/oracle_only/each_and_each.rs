//! Damage to two kinds of permanents, each named: "~ deals 4 damage to each creature and each planeswalker." The
//! renderers write "each creature or planeswalker".

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref};

/// Adds the wordings of damage to each of two card types.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let kinds = ability.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::DealDamage { to: Ref::Each(selector), .. }
                if matches!(selector.0.as_slice(), [Pred::AnyOf(kinds)] if kinds.len() == 2)
        )
    });
    if !kinds {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let at = text.find(" damage to each ")? + " damage to each ".len();
            let (head, tail) = text.split_at(at);
            let (first, rest) = tail.split_once(" or ")?;
            let second = rest.split([' ', ',', '.']).next().unwrap_or("");
            (!first.contains(' ') && !second.is_empty())
                .then(|| format!("{head}{first} and each {rest}"))
        })
        .collect();
    texts.extend(said);
}
