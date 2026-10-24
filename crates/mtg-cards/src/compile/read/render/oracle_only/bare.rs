//! The ability with other effects, for the writers that word a step apart from the sentences around it.

use crate::ability::Ability;
use crate::effect::Effect;

/// The same ability with `effects` as its effects; `None` for an ability that has none.
pub(super) fn with_effects(ability: &Ability, new: Vec<Effect>) -> Option<Ability> {
    let mut bare = ability.clone();
    match &mut bare {
        Ability::Spell { effects, .. }
        | Ability::Activated { effects, .. }
        | Ability::Loyalty { effects, .. }
        | Ability::Triggered { effects, .. } => *effects = new,
        _ => return None,
    }
    Some(bare)
}

/// The sentence with its first letter in lower case.
pub(super) fn lowered(sentence: &str) -> String {
    let mut letters = sentence.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(letters).collect()
    })
}
