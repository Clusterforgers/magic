//! Wordings the oracle text uses for an ability that the renderers write another way, each derived from the wordings of
//! [`super::variants`] by the data of the ability. The compiler's own reads stay on `variants`; only the proposal
//! checker accepts these (`written_variants`).

mod conditions;
mod destroyed_this_way;
mod each_of;
mod either_event;
mod entry_counters;
mod exile_play;
mod land_types;
mod look_split;
mod non_lists;
mod paid_steps;
mod sacrifices;
mod target_and;
mod trailing_if;
mod wheres;

use crate::ability::Ability;

/// Adds to `texts` the oracle wordings of the ability that the renderers do not write.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    conditions::add(ability, texts);
    destroyed_this_way::add(ability, texts);
    each_of::add(ability, texts);
    either_event::add(ability, texts);
    entry_counters::add(ability, texts);
    exile_play::add(ability, texts);
    land_types::add(ability, texts);
    look_split::add(ability, texts);
    non_lists::add(ability, texts);
    paid_steps::add(ability, texts);
    sacrifices::add(ability, texts);
    target_and::add(ability, texts);
    trailing_if::add(ability, texts);
    wheres::add(ability, texts);
}
