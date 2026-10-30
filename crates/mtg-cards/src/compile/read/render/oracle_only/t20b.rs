//! Wordings of replacement effects that the templates do not read: a replacement that triples, and one that sends a
//! player's draw to another player.

mod draw_skip;
mod triple;

use crate::ability::Ability;

/// Adds the oracle wordings of the replacements of this module.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    triple::add(ability, texts);
    draw_skip::add(ability, texts);
}
