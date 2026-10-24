//! Wordings of the second sample of the names A to F that only the checker accepts, each derived from the wordings of
//! [`super::super::variants`] by the data of the ability.

mod all_others;
mod chosen_inline;
mod every_basic;
mod have_lose;
mod investigate_times;
mod no_control;
mod owner_hand;
mod prevent_all;
mod subtype_card;
mod that_controller;
mod that_noun;

use crate::ability::Ability;

/// Adds to `texts` the oracle wordings of the ability that the renderers do not write.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    all_others::add(ability, texts);
    chosen_inline::add(ability, texts);
    every_basic::add(ability, texts);
    have_lose::add(ability, texts);
    investigate_times::add(ability, texts);
    no_control::add(ability, texts);
    owner_hand::add(ability, texts);
    prevent_all::add(ability, texts);
    subtype_card::add(ability, texts);
    that_controller::add(ability, texts);
    that_noun::add(ability, texts);
}
