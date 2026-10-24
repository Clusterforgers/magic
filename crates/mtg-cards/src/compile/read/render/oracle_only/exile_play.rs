//! Cards exiled by a spell that the player may play: "Exile the top five cards of your library. You may play cards
//! exiled this way until the end of your next turn." The renderers word the permission with "Then" first and call the
//! cards "them".

use crate::ability::Ability;

/// The permission as the renderers word it and as the oracle text does.
const PERMISSION: (&str, &str) = (
    ". Then until the end of your next turn, you may play them.",
    ". You may play cards exiled this way until the end of your next turn.",
);

/// Adds the wordings that put the permission in the oracle's words, for a spell that exiles cards and lets its
/// controller play them.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !matches!(ability, Ability::Spell { .. }) {
        return;
    }
    let (said, wanted) = PERMISSION;
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(said))
        .map(|text| text.replacen(said, wanted, 1))
        .collect();
    texts.extend(found);
}
