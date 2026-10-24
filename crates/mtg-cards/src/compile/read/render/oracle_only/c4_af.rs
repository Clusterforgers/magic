//! Wordings of the fourth sample of the names A to F that only the checker accepts: a count of tokens said with "for each",
//! a pronoun for a group after counters, a spell's pump with "where X is", the card in its graveyard, the commander
//! creatures a player owns, a discard of one kind of card and a handful of phrases the renderers write another way.

mod commander_own;
mod discard_kinds;
mod for_each_create;
mod graveyard_static;
mod permanent_cards;
mod phrases;
mod text;
mod they_gain;
mod where_per;

use crate::ability::Ability;

/// Adds the wordings of this sample to `texts`.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for_each_create::add(ability, texts);
    they_gain::add(ability, texts);
    where_per::add(ability, texts);
    permanent_cards::add(ability, texts);
    discard_kinds::add(ability, texts);
    graveyard_static::add(ability, texts);
    commander_own::add(ability, texts);
    phrases::add(ability, texts);
}
